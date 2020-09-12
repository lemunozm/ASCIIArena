use super::session::{Room, SessionCreationResult};
use super::game::{Game};

use crate::message::{ClientMessage, ServerMessage, ServerInfo, LoginStatus};
use crate::version::{self, Compatibility};
use crate::util::{self};

use message_io::events::{EventQueue};
use message_io::network::{NetworkManager, NetEvent, Endpoint};

use std::time::{Duration, Instant};

lazy_static! {
    static ref GAME_STEP_DURATION: Duration = Duration::from_secs_f32(1.0 / 30.0);
}

#[derive(Debug)]
enum Event {
    Network(NetEvent<ClientMessage>),
    CreateGame,
    CreateArena,
    StartArena,
    GameStep,
    Reset,
    Close,
}

pub struct ServerConfig {
    pub tcp_port: u16,
    pub udp_port: u16,
    pub players_number: usize,
    pub map_size: usize,
    pub winner_points: usize,
    pub arena_waiting: Duration,
}

pub struct ServerManager {
    event_queue: EventQueue<Event>,
    network: NetworkManager,
    room: Room<Endpoint>,
    game: Option<Game>,
    timestamp_last_arena_creation: Option<Instant>,
    config: ServerConfig,
}

impl ServerManager {
    pub fn new(config: ServerConfig) -> Option<ServerManager> {
        let mut event_queue = EventQueue::new();

        let network_sender = event_queue.sender().clone();
        let mut network = NetworkManager::new(move |net_event| network_sender.send(Event::Network(net_event)));

        let network_sender = event_queue.sender().clone();
        ctrlc::set_handler(move || network_sender.send_with_priority(Event::Close)).unwrap();

        let network_interface = "0.0.0.0";
        if let Err(_) = network.listen_tcp((network_interface, config.tcp_port)) {
            log::error!("Can not run server on tcp port {}", config.tcp_port);
            return None;
        }

        if let Err(_) = network.listen_udp((network_interface, config.udp_port)) {
            log::error!("Can not run server on udp port {}", config.udp_port);
            return None;
        }

        log::info!("Server running on tcp ports {} (tcp) and {} (udp)", config.tcp_port, config.udp_port);
        Some(ServerManager {
            event_queue,
            network,
            room: Room::new(config.players_number),
            game: None,
            timestamp_last_arena_creation: None,
            config,
        })
    }

    pub fn run(&mut self) {
        loop {
            let event = self.event_queue.receive();
            log::trace!("[Process event] - {:?}", event);
            match event {
                Event::Network(net_event) => match net_event {
                    NetEvent::AddedEndpoint(_) => (),
                    NetEvent::RemovedEndpoint(endpoint) => {
                        self.process_disconnection(endpoint);
                    },
                    NetEvent::Message(endpoint, message) => {
                        log::trace!("Message from {}", endpoint.addr());
                        match message {
                            ClientMessage::Version(client_version) => {
                                self.process_version(endpoint, &client_version);
                            },
                            ClientMessage::RequestServerInfo => {
                                self.process_request_server_info(endpoint);
                            },
                            ClientMessage::Login(player_name) => {
                                self.process_login(endpoint, &player_name);
                            },
                            ClientMessage::Move => {
                                //TODO
                            },
                            ClientMessage::Skill => {
                                //TODO
                            },
                        }
                    },
                },
                Event::CreateGame => {
                    self.process_create_game();
                },
                Event::CreateArena => {
                    self.process_create_arena();
                },
                Event::StartArena => {
                    self.process_start_arena();
                },
                Event::GameStep => {
                    self.process_game_step();
                },
                Event::Reset => {
                    self.process_reset();
                }
                Event::Close => {
                    log::info!("Closing server");
                    break
                },
            }
        }
    }

    fn process_version(&mut self, endpoint: Endpoint, client_version: &str) {
        let compatibility = version::check(&client_version, version::current());
        match compatibility {
            Compatibility::Fully =>
                log::trace!("Fully compatible versions: {}", client_version),
            Compatibility::OkOutdated =>
                log::info!("Compatible client version but differs. Client: {}. Server: {}", client_version, version::current()),
            Compatibility::None =>
                log::warn!("Incompatible client version. Client: {}. Server: {}. Rejected", client_version, version::current()),
        };

        self.network.send(endpoint, ServerMessage::Version(version::current().to_string(), compatibility)).unwrap();
        if let Compatibility::None = compatibility {
            self.network.remove_resource(endpoint.resource_id()).unwrap();
        }
    }

    fn process_request_server_info(&mut self, endpoint: Endpoint) {
        let info = ServerInfo {
            udp_port: self.config.udp_port,
            players_number: self.config.players_number as u8,
            map_size: self.config.map_size as u16,
            winner_points: self.config.winner_points as u16,
            logged_players: self.room.sessions().map(|session| session.name().to_string()).collect(),
        };

        self.network.send(endpoint, ServerMessage::ServerInfo(info)).unwrap();
    }

    fn process_login(&mut self, endpoint: Endpoint, player_name: &str) {
        let status =
        if !util::is_valid_player_name(player_name) {
            log::warn!("Invalid login name '{}' has tried to login", player_name);
            LoginStatus::InvalidPlayerName
        }
        else {
            match self.room.create_session(player_name, endpoint) {
                SessionCreationResult::Created(token) => {
                    let player_names = self.room.sessions().map(|session| session.name().to_string());
                    log::info!("New player logged: {}, current players: {}", player_name, util::format_player_names(player_names));
                    LoginStatus::Logged(token)
                }
                SessionCreationResult::Recycled(token) => {
                    log::info!("Player '{}' reconnected", player_name);
                    LoginStatus::Reconnected(token)
                }
                SessionCreationResult::AlreadyLogged => {
                    log::warn!("Player '{}' has tried to login but the name is already logged", player_name);
                    LoginStatus::AlreadyLogged
                }
                SessionCreationResult::Full => {
                    log::warn!("Player '{}' has tried to login but the player limit has been reached", player_name);
                    LoginStatus::PlayerLimit
                }
            }
        };

        log::trace!("{} with player name '{}' attempts to login. Status: {:?}", endpoint.addr(), player_name, status);
        self.network.send(endpoint, ServerMessage::LoginStatus(status)).unwrap();

        match status {
            LoginStatus::Logged(_) => {
                let endpoints = self.room.safe_endpoints().filter(|&&e| e != endpoint);
                let player_names = self.room.sessions().map(|session| session.name().to_string()).collect();
                self.network.send_all(endpoints, ServerMessage::PlayerListUpdated(player_names)).ok();

                if self.game.is_none() && self.room.is_full() {
                    self.event_queue.sender().send(Event::CreateGame);
                }
            },
            LoginStatus::Reconnected(_) => {
                if self.game.is_some() {
                    self.network.send(endpoint, ServerMessage::StartGame).unwrap();

                    let timestamp = self.timestamp_last_arena_creation.as_ref().unwrap();
                    let duration_since_arena_creation = Instant::now().duration_since(*timestamp);
                    if let Some(waiting) = self.config.arena_waiting.checked_sub(duration_since_arena_creation) {
                        self.network.send(endpoint, ServerMessage::PrepareArena(waiting)).unwrap();
                    }

                    self.network.send(endpoint, ServerMessage::StartArena).unwrap();
                }
            },
            _ => (),
        }
    }

    fn process_create_game(&mut self) {
        log::info!("Starting new game");
        let game = Game::new(self.config.winner_points);
        self.game = Some(game);

        let endpoints = self.room.safe_endpoints();
        self.network.send_all(endpoints, ServerMessage::StartGame).ok();

        self.event_queue.sender().send(Event::CreateArena);
    }

    fn process_create_arena(&mut self) {
        let game = self.game.as_mut().unwrap();
        let arena = game.create_new_arena();
        log::info!("Initializing arena {} in {} seconds...", arena.id(), self.config.arena_waiting.as_secs_f32());

        let endpoints = self.room.safe_endpoints();
        self.network.send_all(endpoints, ServerMessage::PrepareArena(self.config.arena_waiting)).ok();

        self.event_queue.sender().send_with_timer(Event::StartArena, self.config.arena_waiting);
        self.timestamp_last_arena_creation = Some(Instant::now());
    }

    fn process_start_arena(&mut self) {
        let game = self.game.as_ref().unwrap();
        let arena = game.arena().unwrap();
        log::info!("Start arena {}", arena.id());

        let endpoints = self.room.safe_endpoints();
        self.network.send_all(endpoints, ServerMessage::StartArena).ok();

        self.event_queue.sender().send(Event::GameStep);
    }

    fn process_game_step(&mut self) {
        let game = self.game.as_mut().unwrap();
        log::info!("Processing step");

        game.step();

        let arena = game.arena().unwrap();
        let endpoints = self.room.fast_endpoints();
        self.network.send_all(endpoints, ServerMessage::Step).ok();

        if arena.has_finished() {
            let endpoints = self.room.safe_endpoints();
            self.network.send_all(endpoints, ServerMessage::EndArena).ok();

            if game.has_finished() {
                let endpoints = self.room.safe_endpoints();
                self.network.send_all(endpoints, ServerMessage::EndGame).ok();

                self.event_queue.sender().send(Event::Reset);
            }
            else {
                self.event_queue.sender().send(Event::CreateArena);
            }
        }
        else {
            self.event_queue.sender().send_with_timer(Event::GameStep, *GAME_STEP_DURATION);
        }
    }

    fn process_reset(&mut self) {
        self.game = None;
        self.room.clear();
    }

    fn process_disconnection(&mut self, endpoint: Endpoint) {
        if self.game.is_some() {
            if let Some(session) = self.room.notify_lost_endpoint(endpoint) {
                log::info!("Player '{}' disconnected", session.name());
            }
        }
        else {
            if let Some(session) = self.room.remove_session_by_endpoint(endpoint) {
                log::info!("Player '{}' logout, current players: {} ", session.name(),
                    util::format_player_names(self.room.sessions().map(|session| session.name())));

                let player_names = self.room.sessions().map(|session| session.name().to_string()).collect();
                let endpoints = self.room.safe_endpoints();
                self.network.send_all(endpoints, ServerMessage::PlayerListUpdated(player_names)).ok();
            }
        }
    }
}
