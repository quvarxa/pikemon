#![feature(slicing_syntax, box_syntax)]
#![allow(unstable)] // This generates a lot of unnecessary warnings at the moment

extern crate "rustc-serialize" as rustc_serialize;
extern crate common;
extern crate sdl2;
extern crate gb_emu;

use std::cell::RefCell;
use std::io::{File, TcpStream};
use std::sync::mpsc::channel;

use gb_emu::emulator::Emulator;
use gb_emu::cart;
use common::PlayerData;

use interface::InterfaceData;
use net::{NetworkManager, ClientDataManager};
use save::LocalSaveWrapper;

mod client;
mod game;
mod timer;
mod net;
mod font;
mod chat;
mod interface;
mod save;

fn main() {
    let ip_addr = &*std::os::args()[1];
    let socket = TcpStream::connect((ip_addr, 8080)).unwrap();

    let (local_update_sender, local_update_receiver) = channel();
    let (global_update_sender, global_update_receiver) = channel();

    let network_manager = NetworkManager {
        socket: socket,
        local_update_receiver: local_update_receiver,
        global_update_sender: global_update_sender,
    };
    let id = net::handle_network(network_manager).unwrap();

    let interface_data = RefCell::new(InterfaceData::new());
    let mut emulator = box Emulator::new();

    let cart = File::open(&Path::new("Pokemon Red.gb")).read_to_end().unwrap();
    let save_path = Path::new("Pokemon Red.sav");

    let save_file = Box::new(LocalSaveWrapper { path: save_path }) as Box<cart::SaveFile>;
    emulator.load_cart(cart.as_slice(), Some(save_file));
    emulator.start();

    let client_data_manager = ClientDataManager {
        id: id,
        interface_data: &interface_data,
        last_state: PlayerData::new(),
        new_update: false,
        local_update_sender: local_update_sender,
        global_update_receiver: global_update_receiver,
        chat_box: chat::ChatBox::new(),
    };

    if let Err(e) = client::run(client_data_manager, emulator) {
        println!("Pikemon encountered an error and was forced to close. ({})", e);
    }
}
