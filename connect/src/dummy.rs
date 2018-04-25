#![allow(unused)]
extern crate mercury_home_protocol;

extern crate multihash;
extern crate multiaddr;

extern crate tokio_stdin_stdout;
extern crate tokio_core;
extern crate tokio_io;
extern crate futures;

use mercury_home_protocol::*;

use super::*;
use ::net::*;

use std::cell::RefCell;
use std::rc::Rc;
use std::collections::HashMap;
use std::io::{BufRead, Read, Write, stdin};

use multihash::{encode, Hash};
use multiaddr::{Multiaddr, ToMultiaddr};

use tokio_core::reactor;
use tokio_core::net::TcpStream;
use tokio_io::{AsyncRead, AsyncWrite};

use futures::sync::mpsc;
use futures::{future, Async, sync, Future, IntoFuture, Sink, Stream};

pub fn generate_hash( base : &str) -> Vec<u8> {
    encode(Hash::SHA2256, base.as_bytes()).unwrap()
}

pub fn generate_hash_from_vec( base : Vec<u8>) -> Vec<u8> {
    encode(Hash::SHA2256, &base).unwrap()
}

pub struct TestSetup{
    pub homeprofile: Profile,
    pub homeprofileid: ProfileId,
    pub homesigner: Rc<Signer>,
    pub homeaddr: String,
    pub homemultiaddr: Multiaddr,
    pub home: Rc< RefCell< MyDummyHome > >,
    pub user: Profile,
    pub userid: ProfileId,
    pub usersinger: Rc<Signer>,
    pub userownprofile : OwnProfile, 
    pub profilegate: ProfileGatewayImpl,
}

impl TestSetup{
    pub fn setup()-> Self {
        let homesigner = Rc::new(Signo::new("homesigner"));
        let homeaddr = String::from("/ip4/127.0.0.1/udp/9876");
        let homemultiaddr = homeaddr.to_multiaddr().unwrap();
        let homeprofileid =  ProfileId(generate_hash("home"));
        let homeprof = Profile::new_home(ProfileId(generate_hash("home")), homesigner.pub_key().clone(), homemultiaddr.clone());

        let usersigner = Rc::new(Signo::new("Deusz"));
        let userid = ProfileId( generate_hash_from_vec( usersigner.pub_key().0.clone() ) );
        let user = make_own_persona_profile(&usersigner.pub_key().clone());
        let userownprofile = create_ownprofile(user.clone());

        let mut dht = ProfileStore::new();
        dht.insert(homeprofileid.clone(), homeprof.clone());
        let mut home_storage = Rc::new( RefCell::new(dht) );
        let mut store_rc = Rc::clone(&home_storage);
        let homeprof = Profile::new_home(homeprofileid.clone(), homesigner.pub_key().clone(), homemultiaddr.clone());
        let mut home = Rc::new( RefCell::new( MyDummyHome::new( homeprof.clone() , home_storage ) ) );
        let homerc = Rc::clone(&home);

        let profilegateway = ProfileGatewayImpl{
            signer:         usersigner.clone(),
            profile_repo:   store_rc,
            home_connector: Rc::new( dummy::DummyConnector::new_with_home( home ) ),
        };

        Self{
            homeprofile: homeprof,
            homeprofileid: homeprofileid,
            homesigner: homesigner,
            homeaddr: homeaddr,
            homemultiaddr: homemultiaddr,
            home: homerc,
            user: user,
            userid: userid,
            usersinger: usersigner,
            userownprofile: userownprofile,
            profilegate: profilegateway,
        }
    }
}


pub fn create_ownprofile(p : Profile)->OwnProfile{
    OwnProfile::new(&p, &[])
}

pub fn make_own_persona_profile(pubkey : &PublicKey)->Profile{
    let id = generate_hash_from_vec(pubkey.0.clone());
    let empty = vec![];
    let homes = vec![];
    Profile::new(
        &ProfileId(id), 
        &pubkey,
        &[ProfileFacet::Persona( PersonaFacet{ homes : homes , data : empty } )] 
    )
}

pub fn make_home_profile(addr : &str, pubkey : &PublicKey)->Profile{
    let homeaddr = addr.to_multiaddr().unwrap();
    let home_hash = generate_hash_from_vec(pubkey.0.clone());
    let empty = vec![];
    let homevec = vec![homeaddr];
    Profile::new(
        &ProfileId(home_hash), 
        &pubkey,
        &[ProfileFacet::Home( HomeFacet{ addrs : homevec , data : empty } ) ] 
    )
}
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Signo{
    prof_id : ProfileId,
    pubkey : PublicKey,
    privkey : Vec<u8>,
}

impl Signo{
    pub fn new( whatever : &str)->Self{
        Signo{
            prof_id : ProfileId( generate_hash_from_vec( generate_hash(whatever) ) ),
            pubkey : PublicKey( generate_hash(whatever) ),
            privkey : generate_hash(whatever),
        }
    }
}

impl Signer for Signo{
    fn prof_id(&self) -> &ProfileId{
        &self.prof_id
    }
    fn pub_key(&self) -> &PublicKey{
        &self.pubkey
    }
    fn sign(&self, data: &[u8]) -> Signature{
        let mut sig = String::new();
        sig.push_str( std::str::from_utf8(&data).unwrap() );
        sig.push_str( std::str::from_utf8(&self.privkey).unwrap() );
        Signature( sig.into_bytes() )
    }
}

pub fn dummy_relation(rel_type: &str) -> Relation{
    Relation::new(
        &make_own_persona_profile( &PublicKey( Vec::from( "too_hot_today" ) ) ),
        &dummy_relation_proof(rel_type)
    )
}

pub fn dummy_relation_proof(rel_type: &str)->RelationProof{
    RelationProof::new( 
        rel_type, 
        &ProfileId(Vec::from("TestMe")),
        &Signature(Vec::from("TestMe")),
        &ProfileId(Vec::from("TestOther")),
        &Signature(Vec::from("TestOther"))
        )
} 

#[derive(Debug)]
pub struct ProfileStore{
    content : HashMap<ProfileId, Profile>,
}

impl ProfileStore{
    pub fn new() -> Self {
        ProfileStore{
            content : HashMap::new(),
        }
    }
}

impl ProfileStore{
    pub fn insert(&mut self, id : ProfileId, profile : Profile) -> Option<Profile>{
        println!("ProfileStore.add {:?}", id.0);
        self.content.insert(id, profile)
    }

    pub fn get( &self, id : ProfileId ) -> Option<&Profile>{
        self.content.get(&id)
    }
}

impl ProfileRepo for ProfileStore{
    fn list(&self, /* TODO what filter criteria should we have here? */ ) ->
    HomeStream<Profile, String>{
        println!("ProfileStore.list");
        let (send, recv) = futures::sync::mpsc::channel(0);
        recv
    }

    fn load(&self, id: &ProfileId) ->
    Box< Future<Item=Profile, Error=ErrorToBeSpecified> >{
        println!("ProfileStore.load {:?}", id.0);
        //println!("\nProfileStoreContent:::: {:?}", &self.content);
        let prof = self.content.get(&id);
        match prof {
            Some(profile) => {println!("ProfileStore.load.success{:?}", prof);Box::new( future::ok(profile.to_owned()) )},
            None => {println!("ProfileStore.load.fail"); Box::new( future::err(ErrorToBeSpecified::TODO(String::from("ProfileStore/ProfileRepo.load "))) )},
        }
    }

    fn resolve(&self, url: &str) ->
    Box< Future<Item=Profile, Error=ErrorToBeSpecified> >{
        println!("ProfileStore.resolve");
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("ProfileStore/ProfileRepo.resolve "))) )
    }

}

pub struct MyDummyHome{
    pub home_profile   : Profile, 
    pub local_prof_store : HashMap<ProfileId, Vec<u8>>,
    pub storage_layer : Rc<RefCell<ProfileStore>>,
    pub events : HashMap<ProfileId, Vec<ProfileEvent>>,
}

impl MyDummyHome{
    pub fn new(profile : Profile, dht : Rc<RefCell<ProfileStore>>) -> Self {
        println!("MyDummyHome.new");
        MyDummyHome{
            home_profile : profile,
            local_prof_store : HashMap::new(),
            storage_layer : dht,
            events : HashMap::new()
        }
    }
    
    pub fn insert(&mut self, id : ProfileId, profile : Profile)->Option<Profile>{
        println!("MyDummyHome.insert");
        self.storage_layer.borrow_mut().insert(id, profile)
    }
}

impl ProfileRepo for MyDummyHome{
    fn list(&self, /* TODO what filter criteria should we have here? */ ) ->
    HomeStream<Profile, String>{
        println!("MyDummyHome.list");
        let (send, recv) = futures::sync::mpsc::channel(0);
        recv
    }

    fn load(&self, id: &ProfileId) ->
    Box< Future<Item=Profile, Error=ErrorToBeSpecified> >{
        println!("MyDummyHome.load");
        let pr = self.storage_layer.borrow();
        let prof = pr.get(id.to_owned());
        //println!("MyDummyHome.storage_layer.content::::{:?}", &self.storage_layer.borrow().content);
        match prof {
            Some(profile) => Box::new( future::ok(profile.to_owned()) ),
            None => Box::new( future::err(ErrorToBeSpecified::TODO(String::from("MyDummyHome.load "))) ),
        }
    }

    fn resolve(&self, url: &str) ->
    Box< Future<Item=Profile, Error=ErrorToBeSpecified> >{
        println!("MyDummyHome.resolve");
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("MyDummyHome.resolve "))) )
    }
 
}

impl Home for MyDummyHome
{
    // NOTE because we support multihash, the id cannot be guessed from the public key
    fn claim(&self, profile: ProfileId) ->
    Box< Future<Item=OwnProfile, Error=ErrorToBeSpecified> >{
        println!("MyDummyHome.claim");
        match self.storage_layer.borrow().get(profile.clone()){
            Some(own) => {
                match self.local_prof_store.get(&profile){
                        Some(privdata) => Box::new( future::ok( OwnProfile::new( own, privdata ) ) ),
                        None => Box::new( future::ok( OwnProfile::new( own, &Vec::new()) ) )
                }
            },
            None => Box::new( future::err( ErrorToBeSpecified::TODO( String::from( "MyDummyHome.claim" ) ) ) )
        }
    }

    // TODO consider how to enforce overwriting the original ownprofile with the modified one
    //      with the pairing proof, especially the error case
    fn register(&mut self, mut own_prof: OwnProfile, invite: Option<HomeInvitation>) ->
    Box< Future<Item=OwnProfile, Error=(OwnProfile,ErrorToBeSpecified)> >{
        //make some relation magic
        //match own_prof.profile.facets[0].homes.append(dummy_relation(self.home_id));
        println!("MyDummyHome.register {:?}", own_prof);

        let id = own_prof.profile.id.clone();
        let profile = own_prof.profile.clone();
        let mut own_profile = own_prof.clone();
        let mut storing = false;
        for mut facet in own_profile.profile.facets.iter_mut(){
            match facet {
                &mut ProfileFacet::Persona(ref mut persona) => {

                    let relation_proof = RelationProof::new(
                        "home", &profile.id, &Signature(profile.pub_key.0.clone()), &self.home_profile.id, &Signature(self.home_profile.pub_key.0.clone())
                        );
                    
                    persona.homes.append( &mut vec!(relation_proof ) );
                    storing = true;
                },
                _ => {
                    return Box::new( future::err( (own_prof.clone(), 
                    ErrorToBeSpecified::TODO(String::from("MyDummyHome.register fails at finding Persona Facet on given profile "))) ) );
                },
            };
        };

        let mut ret : Box< Future<Item=OwnProfile, Error=(OwnProfile,ErrorToBeSpecified)> > = Box::new( future::err( (own_prof.clone(), ErrorToBeSpecified::TODO(String::from("MyDummyHome.register had unknown error "))) ) );
        
        if storing{
            let ins = self.insert( id.clone(), own_profile.profile.clone() );
            match ins{
                Some(updated) => {
                    ret = Box::new( future::err( (own_prof.clone(), ErrorToBeSpecified::TODO(String::from("MyDummyHome.register already had given profile stored "))) ) );
                },
                None => {
                    println!("MyDummyHome.register.success");
                    self.local_prof_store.insert(id, own_profile.priv_data.clone());
                    ret = Box::new(future::ok(own_profile.clone()));
                },
            }
        }
        println!("register.ret.own_profile {:?} ", own_profile);
        ret
        //own_prof.priv_data = Vec::from("potato");
    }

    // NOTE this closes all previous sessions of the same profile
    fn login(&self, profile: ProfileId) ->
    Box< Future< Item=Box< HomeSession >, Error=ErrorToBeSpecified > >{
        println!("MyDummyHome.login");
        let session = Box::new(HomeSessionDummy::new( profile ,Rc::clone(&self.storage_layer) )) as Box<HomeSession>;
        Box::new( future::ok( session ) )
        //Box::new( future::err(ErrorToBeSpecified::TODO(String::from("MyDummyHome.login "))) )

    } 


    // NOTE acceptor must have this server as its home
    // NOTE empty result, acceptor will connect initiator's home and call pair_response to send PairingResponse event
    fn pair_request(&self, half_proof: RelationHalfProof) ->
    Box< Future<Item=(), Error=ErrorToBeSpecified> >{
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("MyDummyHome.pair_request "))) )
    }

    fn pair_response(&self, rel: RelationProof) ->
    Box< Future<Item=(), Error=ErrorToBeSpecified> >{
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("MyDummyHome.pair_response "))) )
    }

    fn call(&self, rel: RelationProof, app: ApplicationId, init_payload: AppMessageFrame,
            to_caller: Option<AppMsgSink>) ->
        Box< Future<Item=Option<AppMsgSink>, Error=ErrorToBeSpecified> >{
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("MyDummyHome.call "))) )
    }

// TODO consider how to do this in a later milestone
//    fn presence(&self, rel: Relation, app: ApplicationId) ->
//        Box< Future<Item=Option<AppMessageFrame>, Error=ErrorToBeSpecified> >;
}
 
pub struct DummyConnector{
    home : Rc<RefCell<Home>>
}
impl DummyConnector{
    // pub fn new()->Self{
    //     Self{home: Rc::new(MyDummyHome::new())}
    // }

    pub fn new_with_home(home : Rc<RefCell<Home>>)->Self{
        println!("DummyConnector.new_with_home");
        Self{home: home}
    }
}
impl HomeConnector for DummyConnector{
    /// Initiate a permanent connection to the home server defined by `home_profile`, or return an
    /// existing, live `Home` immediately.
    /// `home_profile` must have a HomeFacet with at least an address filled in.
    /// `signer` belongs to me.
    fn connect(&self, home_profile: &Profile, signer: Rc<Signer>) ->
        Box< Future<Item=Rc<RefCell<Home>>, Error=ErrorToBeSpecified> >{
            println!("DummyConnector.connect");
            Box::new( future::ok( Rc::clone( &self.home ) ) )
    }
}

#[derive(Debug)]
pub struct HomeSessionDummy
{
    prof : ProfileId,
    repo : Rc<RefCell<ProfileStore>>
}


impl HomeSessionDummy
{
    pub fn new( prof : ProfileId, repo : Rc<RefCell<ProfileStore>> ) -> Self{ 
        println!("HomeSessionDummy.new");
        Self{ prof : prof, repo : repo } 
    }
}


impl HomeSession for HomeSessionDummy
{
    fn update(&self, own_prof: &OwnProfile) ->
        Box< Future<Item=(), Error=ErrorToBeSpecified> >
    {
        println!("HomeSessionDummy.update");
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("HomeSessionDummy.update "))) )
    }

    // NOTE newhome is a profile that contains at least one HomeFacet different than this home
    // TODO is the ID of the new home enough here or do we need the whole profile?
    fn unregister(&self, newhome: Option<Profile>) ->
        Box< Future<Item=(), Error=ErrorToBeSpecified> >
    {
        // TODO close/drop session connection after successful unregister()
        println!("HomeSessionDummy.unregister");
        Box::new( future::err(ErrorToBeSpecified::TODO(String::from("HomeSessionDummy.unregister "))) )
    }


    fn events(&self) -> Box< HomeStream<ProfileEvent, String> >
    {
        println!("HomeSessionDummy.events");
        let (sender, receiver) = sync::mpsc::channel(0);
        Box::new(receiver)
    }

    // TODO add argument in a later milestone, presence: Option<AppMessageFrame>) ->
    fn checkin_app(&self, app: &ApplicationId) ->
        Box< HomeStream<Call, String> >
    {
        println!("HomeSessionDummy.checkin_app");
        let (sender, receiver) = sync::mpsc::channel(0);
        Box::new(receiver)
    }

    // TODO remove this after testing
    fn ping(&self, txt: &str) ->
        Box< Future<Item=String, Error=ErrorToBeSpecified> >
    {
        println!("Ping received `{}`, sending it back", txt);
        Box::new( future::ok( txt.to_owned() ) )
    }
}

fn main(){
    // let dummy = MyDummyHome::new();
}