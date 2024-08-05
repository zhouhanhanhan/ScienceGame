//! A minimal science game to demonstrate how the smart contract works.


use arrayref::{array_mut_ref, mut_array_refs};
use race_api::prelude::*;
use race_proc_macro::game_handler;
use std::collections::HashMap;
// use race_core;
use serde::{Serialize, Deserialize};
use rsa::{RsaPublicKey, RsaPrivateKey, PaddingScheme, PublicKey};
use rsa::pkcs1::FromRsaPublicKey;
use rsa::pkcs8::FromPublicKey;
use rand::rngs::OsRng;
use std::collections::VecDeque;
use serde_json;

const ACTION_TIMEOUT: u64 = 30_000;
const NEXT_GAME_TIMEOUT: u64 = 15_000;

#[derive(BorshSerialize, BorshDeserialize)]
pub enum GameEvent {
    Submit(Vec<u8>),
    Evaluate(Message),
}

impl CustomEvent for GameEvent {}

#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
struct Message {
    sender: String,
    content: String,
}

// A function for message encryption
fn encrypt_message(message: &Message, public_key: &RsaPublicKey) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // serialization
    let serialized_message = serde_json::to_string(message)?;

    // encryption
    let mut rng = OsRng;
    let encrypted_message = public_key.encrypt(&mut rng, PaddingScheme::new_pkcs1v15_encrypt(), serialized_message.as_bytes())?;
    Ok(encrypted_message)
}

// A function for message decryption
fn decrypt_message(encrypted_message: &[u8], private_key: &RsaPrivateKey) -> Result<Message, Box<dyn std::error::Error>> {
    // decryption
    let decrypted_message = private_key.decrypt(PaddingScheme::new_pkcs1v15_encrypt(), encrypted_message)?;

    // deserialization
    let message: Message = serde_json::from_slice(&decrypted_message)?;

    Ok(message)
}

fn find_player(players: & mut Vec<Player>, addr: String) -> Result<& mut Player, HandleError> {
    for player in players.iter_mut() {
        if player.addr == addr {
            return Ok(player);
        }
    }
    return Err(HandleError::Custom("Player not found".to_string()));
}

// #[derive(BorshDeserialize, BorshSerialize)]
#[derive(Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AccountData {
    pub coin_assigned: u64,
    pub public_key: String,
    pub encrypt_solutions: HashMap<String, String>,
}

#[derive(Default, Debug, BorshSerialize, BorshDeserialize, PartialEq, Eq)]
pub enum GameStage {
    #[default]
    Waiting,
    Submitted,
    Evaluated, 
}

#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct Player {
    pub addr: String,
    pub balance: u64,
    pub local_encrypt_solutions: HashMap<String, String>,
}

#[game_handler]
#[cfg_attr(test, derive(Debug, PartialEq, Eq))]
#[derive(BorshSerialize, BorshDeserialize)]
pub struct ScienceGame {
    pub players: Vec<Player>,
    pub stage: GameStage,
    pub coin_assigned: u64,
    pub public_key: String,
    pub encrypt_solutions: HashMap<String, String>,
    pub tmp_solutions: VecDeque<Vec<u8>>
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct ScienceGameCheckpoint {}

impl ScienceGame {

    fn custom_handle_event(
        &mut self,
        effect: &mut Effect,
        sender: String,
        event: GameEvent,
    ) -> Result<(), HandleError> {
        match event {
            GameEvent::Submit(encrypt_solution) => {
                let mut found = false;
                for player in &self.players {
                    if sender.eq(&player.addr) {
                        found = true;
                    }           
                }  
                if !found {
                    return Err(HandleError::InvalidPlayer);
                }
                self.tmp_solutions.push_back(encrypt_solution);
                self.stage = GameStage::Submitted;
            }

            GameEvent::Evaluate(message) => {
                self.tmp_solutions.pop_front();
                
                let encrypt_solution = message.content;
                if self.encrypt_solutions.contains_key(&encrypt_solution) {
                    self.stage = GameStage::Waiting;
                    println!("Submitted solution already exists");
                    return Ok(());
                }
                let mut player = find_player(& mut self.players, message.sender).unwrap();
                
                player.balance += self.coin_assigned;
                self.encrypt_solutions.insert(encrypt_solution, player.addr.clone());
            
                effect.action_timeout(player.addr.clone(), ACTION_TIMEOUT);

                // Sync solutions to all players
                for player in self.players.iter_mut() {
                    player.local_encrypt_solutions = self.encrypt_solutions.clone()                  
                } 
            }
        }

        Ok(())
    }
}


impl GameHandler for ScienceGame {

    type Checkpoint = ScienceGameCheckpoint;

    fn init_state(_effect: &mut Effect, init_account: InitAccount) -> Result<Self, HandleError> {
        let AccountData {
            coin_assigned,
            public_key, 
            encrypt_solutions,
        } = init_account.data()?;
        let players: Vec<Player> = init_account
            .players
            .into_iter()
            .map(|p| Player {
                addr: p.addr,
                balance: p.balance,
                local_encrypt_solutions: encrypt_solutions.clone(),
            })
            .collect();
        Ok(Self {
            players,
            coin_assigned,
            public_key,
            encrypt_solutions,
            tmp_solutions: VecDeque::new(),
            stage: GameStage::Waiting,
        })
    }

    fn handle_event(&mut self, effect: &mut Effect, event: Event) -> Result<(), HandleError> {
        match event {
            // Custom events are the events we defined for this game particularly
            // See [[GameEvent]].
            Event::Custom { sender, raw } => {
                let event = GameEvent::try_parse(&raw)?;
                self.custom_handle_event(effect, sender, event)?;
            }

            // Sync solutions to any new joint players.
            Event::Sync { new_players, .. } => {
                for p in new_players.into_iter() {
                    self.players.push(Player {
                        addr: p.addr,
                        balance: p.balance,
                        local_encrypt_solutions: self.encrypt_solutions.clone(),
                    });
                }
            }


            _ => (),
        }

        Ok(())
    }

    fn into_checkpoint(self) -> HandleResult<ScienceGameCheckpoint> {
        Ok(ScienceGameCheckpoint {})
    }
}


#[cfg(test)]
mod unit_test;
