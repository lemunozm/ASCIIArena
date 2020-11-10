use super::state::{State, StaticGameInfo, VersionInfo, GameStatus, Arena, ArenaStatus};
use super::server_proxy::{ServerApi, ApiCall, ConnectionStatus, ServerEvent};

use crate::version::{self};

use std::net::{SocketAddr};
use std::time::{Instant};

pub trait AppController {
    fn close(&mut self);
}

/// Action API
#[derive(Debug)]
pub enum Action {
    StartApp,
    Connect(SocketAddr),
    Disconnect,
    Login(char),
    Logout,
    CloseGame,
    Close,
    ServerEvent(ServerEvent),
}

pub struct Store {
    state: State,
    app: Box<dyn AppController>,
    server: ServerApi,
}

impl Store {
    pub fn new(state: State, app: impl AppController + 'static, server: ServerApi) -> Store {
        Store {
            state,
            app: Box::new(app),
            server
        }
    }

    pub fn state(&self) -> &State {
        &self.state
    }

    pub fn dispatch(&mut self, action: Action) {
        log::trace!("Dispatch: {:?}", action);
        match action {
            Action::StartApp => {
                if let Some(addr) = self.state.server.addr {
                    self.server.call(ApiCall::Connect(addr));
                }
            },

            Action::Connect(addr) => {
                self.state.server.addr = Some(addr);
                self.server.call(ApiCall::Connect(addr));
            },

            Action::Disconnect => {
                self.state.server.addr = None;
                self.server.call(ApiCall::Disconnect);
            }

            Action::Login(character) => {
                self.state.user.character = Some(character);
                self.server.call(ApiCall::Login(character));
            },

            Action::Logout => {
                self.state.user.character = None;
                self.state.user.login_status = None;
                self.server.call(ApiCall::Logout);
            }

            Action::CloseGame => {
                self.state.server.game.status = GameStatus::NotStarted;
                self.state.server.game.arena = None;
            }

            Action::Close => {
                self.app.close();
            }

            Action::ServerEvent(server_event) => match server_event {
                ServerEvent::ConnectionResult(status)  => {
                    self.state.server.connection_status = status;
                    if let ConnectionStatus::Connected = status {
                        self.server.call(ApiCall::CheckVersion(version::current().into()));
                    }
                    else {
                        self.dispatch(Action::ServerEvent(ServerEvent::FinishGame));
                        self.state.server.game.arena = None;
                    }
                },

                ServerEvent::CheckedVersion(server_version, compatibility) => {
                    let version_info = VersionInfo { version: server_version, compatibility };
                    self.state.server.version_info = Some(version_info);

                    if compatibility.is_compatible() {
                        self.server.call(ApiCall::SubscribeInfo);
                    }
                },

                ServerEvent::ServerInfo(info) => {
                    let game_info = StaticGameInfo {
                        players_number: info.players_number as usize,
                        map_size: info.map_size as usize,
                        winner_points: info.winner_points as usize,
                    };
                    self.state.server.udp_port = Some(info.udp_port);
                    self.state.server.game_info = Some(game_info);
                    self.state.server.logged_players = info.logged_players;

                    if let Some(character) = self.state.user.character {
                        self.server.call(ApiCall::Login(character));
                    }
                },

                ServerEvent::PlayerListUpdated(player_names) => {
                    self.state.server.logged_players = player_names;
                },

                ServerEvent::LoginStatus(status) => {
                    self.state.user.login_status = Some(status);
                },

                ServerEvent::UdpReachable(value) => {
                    self.state.server.udp_confirmed = Some(value);
                },

                ServerEvent::StartGame => {
                    self.state.server.game.status = GameStatus::Started;
                },

                ServerEvent::FinishGame => {
                    self.state.server.game.status = GameStatus::Finished;
                    self.state.server.logged_players = Vec::new();
                    self.state.server.udp_confirmed = None;
                    self.state.user.character = None;
                    self.state.user.login_status = None;
                },

                ServerEvent::PrepareArena(duration) => {
                    self.state.server.game.next_arena_timestamp = Some(
                        Instant::now() + duration
                    );
                },

                ServerEvent::StartArena(number) => {
                    self.state.server.game.next_arena_timestamp = None;
                    self.state.server.game.arena = Some(Arena {
                        number: number as usize,
                        status: ArenaStatus::Playing,
                    });
                },

                ServerEvent::FinishArena => {
                    let arena = self.state.server.game.arena.as_mut().unwrap();
                    arena.status = ArenaStatus::Finished;
                },

                ServerEvent::ArenaStep => {
                    //TODO
                },
            },
        }
    }
}