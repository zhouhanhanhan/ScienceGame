//! Minimal unit tests to demonstrate how the smart contract works.
//!
//! Three unit tests:
//! 1. Player joins the game
//! 2. Player submits a valid solution
//! 3. Player submits an existing solution

use crate::{AccountData, ScienceGame, GameEvent, GameStage, Player, Message, encrypt_message, decrypt_message};
use race_api::prelude::*;
use race_test::prelude::*;
use rsa::{RsaPublicKey, RsaPrivateKey, PaddingScheme, PublicKey};
use rsa::pkcs1::{FromRsaPublicKey, ToRsaPublicKey};
use rsa::pkcs8::{FromPublicKey, ToPublicKey};
use rand::rngs::OsRng;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[test]
fn test() -> anyhow::Result<()> {
    // Initialize player client, which simulates the behavior of player.
    // TODO: SET player i to real address
    let mut players = Vec::new();

    for i in 1..=10 {
        let player_name = format!("player{}", i);
        let player = TestClient::player(&player_name);
        players.push(player);
    }

    // Print out the initialized players' information.
    for player in &players {
        println!("{:?}", player.get_addr());
    }

    // Initialize the client, which simulates the behavior of transactor.
    // TODO: Now, set our address as transactor. Later can be changed to other players' addresses.
    // TODO: SET transactor to real address
    let mut transactor = TestClient::transactor("transactor");

    // generate public_key and private key for transactors
    // ToDo: load keys from files
    let mut rng = OsRng;
    let bits = 2048;
    let private_key = RsaPrivateKey::new(&mut rng, bits)?;
    let public_key = RsaPublicKey::from(&private_key);

    // Initialize the game account, with 1 player joined.
    // The game account must be served, so we add one server which is the transactor.
    let account_data = AccountData {
        coin_assigned: 1,
        public_key: public_key.to_public_key_pem().expect("Failed to encode public key to PEM"),
        encrypt_solutions: [
            ("13127340485816396534", "player5"),
            ("931693190773671174", "player7"),
            ("15055389790003629452", "player2"),
            ("5091859096217936959", "player2"),
        ]
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect(),
    };

    println!("Create game account");
    let game_account = TestGameAccountBuilder::default()
        .set_transactor(&transactor)
        .add_player(&players[0], 0)
        .with_max_players(1000)
        .with_data(account_data)
        .build();
    let transactor_addr = game_account.transactor_addr.as_ref().unwrap().clone();
    // Print out the initialized Transactor's information.
    println!("Transactor's Address: {:?}", transactor_addr);
    {
        let acc_data: AccountData = AccountData::try_from_slice(&game_account.data)
        .expect("Failed to deserialize account data");
        println!("Initialized Hash solutions: {:?}", acc_data.encrypt_solutions);
    }

    // Create game context and test handler.
    // Initialize the handler state with game account.
    println!("Initialize handler state");
    let mut ctx = GameContext::try_new(&game_account)?;
    let mut handler = TestHandler::init_state(&mut ctx, &game_account)?;
    println!("Players in game: {:?}", ctx.count_players());

    // Start game
    println!("Start game");
    let first_event = ctx.gen_start_game_event();
    let ret = handler.handle_event(&mut ctx, &first_event);
    // println!("Ret: {:?}", ret);
    
    {
        let state: &ScienceGame = handler.get_state();
        println!("State: {:?}", state.stage);
    }


    // Unit Test 1. Player joins the game.
    // Now we have enough players, an event of `GameStart` should be dispatched.
    println!("==================================");
    println!("Unit Test 1: Player2 join the game");
    let av = ctx.get_access_version() + 1;
    let sync_event = Event::Sync {
        new_players: vec![PlayerJoin {
            addr: players[1].get_addr().into(),
            balance: 0,
            position: 1,
            access_version: av,
            verify_key: "".into(),
        }],
        new_servers: vec![],
        transactor_addr: transactor.get_addr(),
        access_version: av,
    };

    handler.handle_event(&mut ctx, &sync_event)?;
    println!("Players in game: {:?}", ctx.count_players());
    println!("GameStatus: {:?}", ctx.get_status());
    {
        let state: &ScienceGame = handler.get_state();
        println!("State: {:?}", state.stage);
        // Check: Sync solution has been updated to all users
        for player in &state.players {
            assert_eq!(state.encrypt_solutions, player.local_encrypt_solutions);
            println!("{:?} local hash solutions: {:?}", player.addr, player.local_encrypt_solutions);
        }
    }

    // Unit Test 2. Player submits a valid solution
    println!("========================================================");
    println!("Unit Test 2: Player 1 prepare to submit a valid solution");
    let message = Message {
        sender: players[0].get_addr(),
        content: "Solution10".to_string(),
    };

    // use transactor's public key to encrypt the message
    {
        let state: &ScienceGame = handler.get_state();
        let public_key = RsaPublicKey::from_public_key_pem(&state.public_key).expect("Failed to obtain public key");
        let encrypt_solution = encrypt_message(&message, &public_key).expect("Failed to obtain public key");
        // println!("Player 1 encrypts solution using transactor's public key: {:?}", encrypt_solution);

        let event = players[0].custom_event(GameEvent::Submit(encrypt_solution));
        handler.handle_event(&mut ctx, &event)?; 
    }
    // Verify tmp solution queue is not empty
    {
        let state: &ScienceGame = handler.get_state();
        let onchain_tmp_solutions = state.tmp_solutions.clone().pop_front().unwrap();
        assert!(onchain_tmp_solutions.len() > 0);
        println!("Current tmp solution queue: {:?}", onchain_tmp_solutions);
    }

    // Transactor evaluate the submission
    println!("Transactor evaluates the solution");
    {
        let state: &ScienceGame = handler.get_state();
        println!("Current encrypted solutions: {:?}", state.encrypt_solutions);
        println!("State: {:?}", state.stage);

        let mut tmp_solutions = state.tmp_solutions.clone();

        let encrypt_solution = tmp_solutions.pop_front().unwrap();

        let decrypt_solution = decrypt_message(&encrypt_solution, &private_key).expect("decrypt_message error");

        println!("decrypt_solution sender: {:?}", decrypt_solution.sender);
        println!("decrypt_solution: {:?}", decrypt_solution.content);

        println!("Transactor evaluate the hash solution");
        let mut hasher = DefaultHasher::new();
        decrypt_solution.content.hash(&mut hasher);
        let hash_solution = hasher.finish();

        let eval_message = Message {
            sender: decrypt_solution.sender,
            content: hash_solution.to_string(),
        };

        let event = transactor.custom_event(GameEvent::Evaluate(eval_message));
        handler.handle_event(&mut ctx, &event)?; 
        let state: &ScienceGame = handler.get_state();
        println!("Encrypted solutions: {:?}", state.encrypt_solutions);
        println!("State: {:?}", state.stage);        
    }

    // Evaluate all players' solution being updated
    {
        let state: &ScienceGame = handler.get_state();
        println!("State: {:?}", state.stage);
        // Check: Sync solution has been updated to all users
        for player in &state.players {
            assert_eq!(state.encrypt_solutions, player.local_encrypt_solutions);
            println!("{:?} local hash solutions: {:?}", player.addr, player.local_encrypt_solutions);
        }
    }

    // Unit Test 3. Player submits an existing solution
    println!("================================================");
    println!("Unit Test 3: Player 2 prepare to submit an existing solution");
    let message = Message {
        sender: players[1].get_addr(),
        content: "Solution10".to_string(),
    };

    // use transactor's public key to encrypt the message
    {
        let state: &ScienceGame = handler.get_state();
        let public_key = RsaPublicKey::from_public_key_pem(&state.public_key).expect("Failed to obtain public key");
        let encrypt_solution = encrypt_message(&message, &public_key).expect("Failed to obtain public key");
        let event = players[0].custom_event(GameEvent::Submit(encrypt_solution));
        handler.handle_event(&mut ctx, &event)?;
    }

    // Verify tmp solution queue is not empty
    {
        let state: &ScienceGame = handler.get_state();
        let onchain_tmp_solutions = state.tmp_solutions.clone().pop_front().unwrap();
        assert!(onchain_tmp_solutions.len() > 0);
        println!("Current tmp solution queue: {:?}", onchain_tmp_solutions);
    }

    // Transactor evaluate the submission
    println!("Transactor evaluates the solution");
    {
        let state: &ScienceGame = handler.get_state();
        println!("Current encrypted solutions: {:?}", state.encrypt_solutions);
        println!("State: {:?}", state.stage);

        let mut tmp_solutions = state.tmp_solutions.clone();

        let encrypt_solution = tmp_solutions.pop_front().unwrap();

        let decrypt_solution = decrypt_message(&encrypt_solution, &private_key).expect("decrypt_message error");

        println!("decrypt_solution sender: {:?}", decrypt_solution.sender);
        println!("decrypt_solution: {:?}", decrypt_solution.content);

        println!("Transactor evaluate the hash solution");
        let mut hasher = DefaultHasher::new();
        decrypt_solution.content.hash(&mut hasher);
        let hash_solution = hasher.finish();

        let eval_message = Message {
            sender: decrypt_solution.sender,
            content: hash_solution.to_string(),
        };

        let event = transactor.custom_event(GameEvent::Evaluate(eval_message));
        handler.handle_event(&mut ctx, &event)?;
        let state: &ScienceGame = handler.get_state();
        println!("Encrypted solutions: {:?}", state.encrypt_solutions);
        println!("State: {:?}", state.stage);
    }

    // Evaluate all players' solution not being updated
    {
        let state: &ScienceGame = handler.get_state();
        println!("State: {:?}", state.stage);
        // Check: Sync solution has been updated to all users
        for player in &state.players {
            assert_eq!(state.encrypt_solutions, player.local_encrypt_solutions);
            println!("{:?} local hash solutions: {:?}", player.addr, player.local_encrypt_solutions);
        }
    }


    Ok(())
}