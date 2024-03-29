use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

use actix_web::Error;
use serde;
use serde::{Deserialize, Serialize};
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
        State::new(Duration::from_secs(30 * 60))
    }
}

impl State {
    pub fn new(timeout: Duration) -> Self {
        State {
            value: HashMap::new(),
            timeout,
            last_use_time: SystemTime::now(),
        }
    }

    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>, Error> {
        if let Some(s) = self.value.get(key) {
            Ok(Some(serde_json::from_str(s)?))
        } else {
            Ok(None)
        }
    }

    pub fn set<T: Serialize>(&mut self, key: &str, value: &T) {
        self.value.insert(key.to_owned(), serde_json::to_string(value).unwrap());
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

    pub fn timeout(&self) -> Duration {
        self.timeout
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
    timeout: Duration,
    started: bool,
}

impl ServerSessionState {
    pub fn new() -> Self {
        ServerSessionState {
            state: Arc::new(RwLock::new(HashMap::new())),
            started: false,
            timeout: Duration::from_secs(30 * 60),
        }
    }

    pub fn start(&mut self) {
        if self.started {
            return;
        }
        let inner = self.state.clone();
        thread::spawn(move || {
            loop {
                inner.write().unwrap().retain(|_, value| {
                    let state: State = serde_json::from_str(value).unwrap();
                    println!("timeout {}", state.timeout.as_secs());
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

    pub fn new_state(&self) -> State {
        State::new(self.timeout)
    }

    pub fn set_state(&mut self, key: &String, state: &State) -> Result<(), Error> {
        let str = serde_json::to_string(state)?;
        self.state.write().unwrap().insert(key.to_string(), str);
        Ok(())
    }

    pub fn remove_state(&mut self, key: &String) {
        self.state.clone().write().unwrap().remove(key).unwrap();
    }

    pub fn set_timeout(&mut self, minutes: u64) {
        self.timeout = Duration::from_secs(minutes * 60)
    }
}