#[macro_use]
extern crate lazy_static;
extern crate derive_new;
#[macro_use]
extern crate derive_builder;

mod client;
mod server;

mod logger;
mod version;
mod message;
mod encoding;
mod util;
mod direction;
mod vec2;
mod character;
mod ids;
mod specification;

use clap::{self, App, AppSettings};

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    // If the user do not specify a subcommand, then run automatically the client mode with no args.
    // This facilitates the usage for distributables.
    if args.len() == 1 {
        args.push("client".into());
    }

    let matches = App::new(clap::crate_name!())
        .version(clap::crate_version!())
        .author(clap::crate_authors!())
        .about(clap::crate_description!())
        .setting(AppSettings::ArgRequiredElseHelp)
        .subcommand(client::configure_cli())
        .subcommand(server::configure_cli())
        .get_matches_from(args);

    match matches.subcommand() {
        ("client", Some(matches)) => client::run(matches),
        ("server", Some(matches)) => server::run(matches),
        _ => unreachable!(),
    }
}
