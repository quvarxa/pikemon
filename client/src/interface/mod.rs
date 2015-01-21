//! Module for interfacing with the emulator
use std::collections::RingBuf;
use std::collections::HashMap;

use gb_emu::cpu::Cpu;
use gb_emu::mmu::Memory;

use common::{PlayerData, PlayerId};
use common::data::{self, Party};

pub mod offsets;
pub mod values;
pub mod extract;
pub mod text;

fn load_party(party: data::Party, mem: &mut Memory) {
    let pokemon = party.pokemon;
    let pokemon_array = [pokemon.0, pokemon.1, pokemon.2, pokemon.3, pokemon.4, pokemon.5];

    let mut addr = (offsets::PROF_OAK_DATA_ADDR & 0x3FFF) as usize;
    let bank = offsets::PROF_OAK_DATA_BANK;

    mem.cart.rom[bank][addr] = 0xFF;
    addr += 1;
    for mon in pokemon_array.iter().take(party.num_pokemon as usize) {
        mem.cart.rom[bank][addr] = mon.level;
        mem.cart.rom[bank][addr + 1] = mon.species;
        addr += 2;
    }
    mem.cart.rom[bank][addr] = 0;
}

#[derive(PartialEq)]
enum DataState {
    Normal,
    Hacked,
}

#[derive(PartialEq)]
pub enum GameState {
    Normal,
    Waiting,
}

#[derive(PartialEq)]
pub enum NetworkRequest {
    None,
    Battle(PlayerId),
}

pub struct GameData {
    pub game_state: GameState,
    pub network_request: NetworkRequest,
    pub other_players: HashMap<u32, PlayerData>,
    last_interaction: u32,
    sprite_id_state: DataState,
    text_state: DataState,
    current_message: RingBuf<u8>,
}

impl GameData {
    pub fn new() -> GameData {
        GameData {
            game_state: GameState::Normal,
            network_request: NetworkRequest::None,
            other_players: HashMap::new(),
            last_interaction: 0,
            sprite_id_state: DataState::Normal,
            text_state: DataState::Normal,
            current_message: RingBuf::new(),
        }
    }

    pub fn create_message_box(&mut self, input: &str) {
        self.current_message.push_back(text::special::TEXT_START);
        self.current_message.extend(text::Encoder::new(input));
        self.current_message.push_back(text::special::END_MSG);
        self.current_message.push_back(text::special::TERMINATOR);
    }
}

pub fn sprite_check_hack(cpu: &mut Cpu, mem: &mut Memory, game_data: &mut GameData) {
    if cpu.pc == offsets::OVERWORLD_LOOP_START {
        game_data.sprite_id_state = DataState::Normal;
    }

    if (cpu.pc == offsets::SPRITE_CHECK_EXIT_1 && mem.lb(offsets::NUM_SPRITES) == 0) ||
        cpu.pc == offsets::SPRITE_CHECK_EXIT_2
    {
        let map_id = mem.lb(offsets::MAP_ID);

        // Determine the tile that the player is trying to move into.
        let mut x = mem.lb(offsets::MAP_X);
        let mut y = mem.lb(offsets::MAP_Y);
        match mem.lb(offsets::PLAYER_DIR) {
            0x00 => y += 1, // Down
            0x04 => y -= 1, // Up
            0x0C => x += 1, // Right
            _    => x -= 1, // Left
        }

        // Check if there are any other players that occupy this tile
        for (id, player) in game_data.other_players.iter() {
            if player.movement_data.map_id == map_id && player.check_collision(x, y) {
                // If there was a player set a sentinel value so the game thinks that there is
                // something in the way.
                mem.sb(offsets::SPRITE_INDEX, 0xFF);
                game_data.sprite_id_state = DataState::Hacked;
                game_data.last_interaction = *id;
                break;
            }
        }
    }
}

pub fn display_text_hack(cpu: &mut Cpu, mem: &mut Memory, game_data: &mut GameData) {
    if game_data.sprite_id_state == DataState::Hacked &&
        cpu.pc == offsets::DISPLAY_TEXT_ID_AFTER_INIT
    {
        // Skip unnecessary parts of the DISPLAY_TEXT_ID routine releated to finding the correct
        // message address when we are interacting with a hacked object.
        cpu.jump(offsets::DISPLAY_TEXT_SETUP_DONE);
        // Set the delay time (this is normally set in the middle of the code we just skipped)
        mem.sb(offsets::FRAME_COUNTER, 30);

        game_data.text_state = DataState::Hacked;
        game_data.create_message_box("PLAYER has nothing\nto say.");

        game_data.network_request = NetworkRequest::Battle(game_data.last_interaction);
        // We probably want to defer this until as late as possible, to avoid latency causing too
        // much of an issue
        game_data.game_state = GameState::Waiting;
    }

    // If the text state is hacked when running the text processor, read from our message buffer
    // instead of from the emulator's memory
    if game_data.text_state == DataState::Hacked && (cpu.pc == offsets::TEXT_PROCESSOR_NEXT_CHAR_1
        || cpu.pc == offsets::TEXT_PROCESSOR_NEXT_CHAR_2)
    {
        cpu.a = game_data.current_message.pop_front().unwrap_or(text::special::TERMINATOR);
        cpu.pc += 1;
    }

    // Ensure that when we leave the text processor, we reset the text state so that the next call
    // to the text processor will correctly read from the game.
    if cpu.pc == offsets::TEXT_PROCESSOR_END {
        game_data.text_state = DataState::Normal;
    }
}

// A temporary method to set a battle. In future we probably want to do more of the setup manually
// so that we can do things like set the pokemon moves, EVs, DVs etc.
pub fn set_battle(mem: &mut Memory, party: Party) {
    mem.sb(offsets::BATTLE_TYPE, values::BattleType::Normal as u8);
    mem.sb(offsets::ACTIVE_BATTLE, values::ActiveBattle::Trainer as u8);
    mem.sb(offsets::TRAINER_NUM, 1);
    mem.sb(offsets::CURRRENT_OPPONENT, values::TrainerClass::ProfOak as u8 + values::TRAINER_TAG);

    load_party(party, mem);
}


