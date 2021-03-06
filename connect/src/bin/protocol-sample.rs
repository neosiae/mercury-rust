#[macro_use]
extern crate clap;
extern crate failure;
extern crate futures;
#[macro_use]
extern crate log;
extern crate log4rs;
extern crate multiaddr;
//extern crate multihash;
extern crate tokio_core;

extern crate mercury_connect;
extern crate mercury_home_protocol;



use std::net::SocketAddr;
use std::rc::Rc;
use std::time::Duration;

use clap::App;
use failure::Fail;
use futures::prelude::*;
use multiaddr::ToMultiaddr;
use tokio_core::reactor;

use mercury_connect::*;
use mercury_connect::profile::MyProfileImpl;
use mercury_home_protocol::*;
use mercury_home_protocol::crypto::*;



fn main()
{
    log4rs::init_file( "log4rs.yml", Default::default() ).unwrap();
    // TODO use structopt instead of yaml+clap here
    let yaml = load_yaml!("protocol-sample-cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    let client_private_key_file = matches.value_of("client-key-file").unwrap();
    let client_private_key = PrivateKey(std::fs::read(client_private_key_file).unwrap());
    let client_signer = Rc::new( Ed25519Signer::new(&client_private_key).unwrap() );
    let client_facet = ProfileFacet::Persona(PersonaFacet {homes: vec![], data: vec![]});
    let client_profile = Profile::new(&client_signer.profile_id(), &client_signer.public_key(), &client_facet);
    let client_own_profile = OwnProfile::new(&client_profile, &vec![]);

    // server details has to be taken from the command line
    // we need 3 pieces of information
    // (1) ProfileId
    // (2) Public key hash
    // (3) Address (since we don't yet have access to ipfs)
    let server_key_file = matches.value_of("server-key-file").unwrap();
    let srv_addr : SocketAddr = matches.value_of("server-addr").unwrap().parse().expect("Failed to parse server address");
    let addr = srv_addr.to_multiaddr().expect("Failed to parse server address");    

    let server_key = PublicKey(std::fs::read(server_key_file).unwrap());
    info!("homenode public key: {:?}", server_key);
    let server_id = ProfileId::from(&server_key);            
    info!("homenode profile id: {:?}", server_id);
    let home_profile = Profile::new_home(server_id.clone(), server_key, addr);

    let profile_store = SimpleProfileRepo::default();
    profile_store.insert(home_profile);

    let mut reactor = reactor::Core::new().unwrap();
    let home_connector = SimpleTcpHomeConnector::new(reactor.handle());
    let profile_gw = MyProfileImpl::new( client_own_profile.clone(), client_signer.clone(), Rc::new(profile_store),
        Rc::new(home_connector), reactor.handle() );
    let test_fut = profile_gw.connect_home(&server_id.clone())
        .map_err(|err| err.context(::mercury_home_protocol::error::ErrorKind::ConnectionToHomeFailed).into())
        .and_then(|home| {
            info!("connected, registering");
            let halfproof = RelationHalfProof::new(
                RelationProof::RELATION_TYPE_HOSTED_ON_HOME, &server_id, &*client_signer);
            home.register(client_own_profile, halfproof, None)
                .map(|own_profile| (own_profile, home) )
                .map_err( |(_own_profile, e)| e )
        })
        .and_then(move |(own_profile, home)| {
            info!("registered, logging in");
            let home_proof = match own_profile.profile.facet {
                ProfileFacet::Persona(persona) => persona.homes.get(0).map(|item| item.to_owned()),
                _ => None
            };
            match home_proof {
                Some(proof) => home.login(&proof),
                None => Box::new( Err(::mercury_home_protocol::error::ErrorKind::LoginFailed.into()).into_future())
            }
        })
        .and_then(|session| {
            info!("session created, sending ping");
            session.ping("hahoooo")
        })
        .map(|pong| {
            info!("received pong");
            pong
        });

    let pong = reactor.run(test_fut);
    
    debug!("Response: {:?}", pong);

    let timeout = reactor::Timeout::new( Duration::from_secs(5), &reactor.handle() ).unwrap();
    let result = reactor.run(timeout);
    info!("Client result {:?}", result);
}