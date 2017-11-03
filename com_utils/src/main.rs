#![feature(try_trait)]

#[macro_use]
extern crate clap;
#[macro_use]
extern crate if_chain;
extern crate syn;
extern crate glob;
extern crate com_common;

mod idl;

use clap::{App, AppSettings, SubCommand, Arg, ArgMatches};
use std::error::Error;

pub struct AppError( String );
pub type AppResult = Result< (), AppError >;

impl From<String> for AppError {
    fn from( e : String ) -> AppError {
        AppError( e )
    }
}

impl From<glob::PatternError> for AppError {
    fn from( e : glob::PatternError ) -> AppError {
        AppError( String::from( e.description() ) )
    }
}

impl From<std::io::Error> for AppError {
    fn from( e : std::io::Error ) -> AppError {
        AppError( String::from( e.description() ) )
    }
}

impl std::fmt::Display for AppError {
    fn fmt( &self, f: &mut std::fmt::Formatter ) -> std::fmt::Result {
        write!( f, "{}", self.0 )
    }
}

fn main() {
    let matches = App::new( "Rust COM utility" )
            .version( "0.1" )
            .author( "Mikko Rantanen <rantanen@jubjubnest.net>" )
            .setting( AppSettings::SubcommandRequiredElseHelp )
            .subcommand( SubCommand::with_name( "idl" )
                .about( "Generates IDL file from the Rust crate" )
                .version( crate_version!() )
                .arg( Arg::with_name( "path" )
                   .help( "Path to the crate to process" )
                   .default_value( "." )
                   .index( 1 )
                )
            )
        .get_matches();

    if let Err( e ) = match matches.subcommand() {
        ( "idl", Some( idl_matches ) ) => { idl::run( idl_matches ) },
        _ => unreachable!(),
    } {
        eprintln!( "{}", e );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
