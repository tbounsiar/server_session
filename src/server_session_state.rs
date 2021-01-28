use std::borrow::{Borrow, BorrowMut};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::thread::{JoinHandle, sleep};
use std::time::Duration;
use std::time::SystemTime;

use actix_web::Error;
use serde;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde::de::DeserializeOwned;
use serde_millis;

#[derive(Serialize, Deserialize)]
pub struct State {
    value: HashMap<String, String>,
    #[serde(with = "serde_millis")]
    timeout: Duration,
    #[serde(with = "serde_millis")]
    last_use_time: SystemTime,
}

impl Default for State {
    fn default() -> Self {
        State {
            value: HashMap::new(),
            timeout: Duration::from_secs(10),
            last_use_time: SystemTime::now(),
        }
    }
}

impl State {
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        if let Some(s) = self.value.get(key) {
            Ok(Some(serde_json::from_str(s)?))
        } else {
            Ok(None)
        }
    }

    pub fn set<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), Error> {
        self.value.insert(key.to_owned(), serde_json::to_string(value)?);
        Ok(())
    }

    pub fn remove(&mut self, key: &str) {
        self.value.remove(key);
    }

    pub fn clear(&mut self) {
        self.value.clear();
    }

    pub fn extend(&mut self, data: State) {
        self.value.extend(data.value);
    }

    pub fn update_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn update_last_use_time(&mut self) {
        self.last_use_time = SystemTime::now();
    }

    pub fn is_expired(&self) -> bool {
        SystemTime::now() > self.last_use_time + self.timeout
    }
}

pub struct ServerSessionState {
    state: Arc<RwLock<HashMap<String, String>>>,
    started: bool,
}

impl ServerSessionState {
    pub fn new() -> Self {
        ServerSessionState {
            state: Arc::new(RwLock::new(HashMap::new())),
            started: false,
        }
    }

    pub fn start(&mut self) {
        if self.started {
            return;
        }
        let mut inner = self.state.clone();
        thread::spawn(move || {
            loop {
                inner.write().unwrap().retain(|key, value| {
                    let state: State = serde_json::from_str(value).unwrap();
                    !state.is_expired()
                });
                thread::sleep(Duration::from_secs(1));
            }
        });
        self.started = true;
    }

    pub fn get_state(&self, key: &String) -> Option<State> {
        if let Some(s) = self.state.clone().read().unwrap().get(key) {
            match serde_json::from_str(s) {
                Ok(state) => Some(state),
                Err(_) => None
            }
        } else {
            None
        }
    }

    pub fn set_state(&mut self, key: &String, state: &State) -> Result<(), Error> {
        let str = serde_json::to_string(state)?;
        self.state.write().unwrap().insert(key.to_string(), str);
        Ok(())
    }

    pub fn remove_state(&mut self, key: &String) -> Result<(), Error> {
        match self.state.clone().write().unwrap().remove(key) {
            Some(s) => {}
            None => {}
        };
        Ok(())
    }
}