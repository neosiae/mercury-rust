use super::*;

use std::cell::RefCell;
use std::rc::Rc;

use futures::{Future, IntoFuture, sync::mpsc};

use sdk::*;
//use mercury_home_protocol::*;
use mercury_storage::async::KeyValueStore;



pub struct DAppConnect
{
    gateway: Rc<ProfileGateway>,
    app:     ApplicationId,
    session: Rc<RefCell< Option<Rc<HomeSession>> >>,
}


impl DAppConnect
{
    pub fn new(gateway: Rc<ProfileGateway>, app: &ApplicationId) -> Self
        { Self{ gateway, app: app.to_owned(), session: Rc::new( RefCell::new(None) ) } }


    fn login(&self) -> Box< Future<Item=Rc<HomeSession>, Error=ErrorToBeSpecified> >
    {
        if let Some(ref session_rc) = *self.session.borrow()
            { return Box::new( Ok( session_rc.clone() ).into_future() ) }

        let login_fut = self.gateway.login()
            .map( {
                let session_cache = self.session.clone();
                move |session| {
                    *session_cache.borrow_mut() = Some( session.clone() );
                    session
                }
            } );
        Box::new(login_fut)
    }


    // Try fetching RelationProof from existing contacts. If no appropriate contact found,
    // initiate a pairing procedure and return when it's completed, failed or timed out
    fn get_relation_proof(&self, profile_id: &ProfileId)
        -> Box< Future<Item=Relation, Error=ErrorToBeSpecified>>
    {
        let my_id = self.gateway.signer().profile_id().to_owned();
        let profile_id = profile_id.to_owned();
        let gateway = self.gateway.clone();
        let res_fut = self.contacts()
            .and_then( move |contacts|
            {
                let first_match = contacts.iter()
                    .filter( move |relation| relation.proof.peer_id(&my_id).map(|id| id.to_owned()) == Ok(profile_id.clone()) )
                    .nth(0);
                match first_match {
                    Some(relation) => Ok( relation.to_owned() ).into_future(),
                    None =>
// TODO how to receive notification on incoming pairing response without keeping a session alive and consuming the whole event stream?
                        Err( ErrorToBeSpecified::TODO( "get_relation_proof: no appropriate relation found".to_string()) ).into_future()
//                        gateway.pair_request(RelationProof::RELATION_TYPE_ENABLE_CALLS_BETWEEN, &profile_id, None)
//                            .then( |_| unimplemented!() )
                }
            } );
        Box::new(res_fut)
    }
}


// TODO this aims only feature-completeness initially for a HelloWorld dApp,
//      but we also have to include security with authorization and UI-plugins later
impl DAppApi for DAppConnect
{
    fn selected_profile(&self) -> &ProfileId
        { self.gateway.signer().profile_id() }


    fn contacts(&self) -> Box< Future<Item=Vec<Relation>, Error=ErrorToBeSpecified> >{
        unimplemented!();
    }


    fn app_storage(&self) -> Box< Future<Item=KeyValueStore<String,String>, Error=ErrorToBeSpecified> >{
        unimplemented!();
    }


    fn checkin(&self) -> Box< Future<Item=HomeStream<Box<IncomingCall>,String>, Error=ErrorToBeSpecified> >
    {
        let checkin_fut = self.login()
            .and_then( {
                let app = self.app.clone();
                move |session| Ok( session.checkin_app(&app) )
            } );
        Box::new(checkin_fut)
    }


    fn call(&self, profile_id: &ProfileId, init_payload: AppMessageFrame)
        -> Box< Future<Item=Call, Error=ErrorToBeSpecified> >
    {
        let call_fut = self.get_relation_proof(profile_id)
            .and_then( {
                let gateway = self.gateway.clone();
                let app_id = self.app.clone();
                let (to_caller, from_callee) = mpsc::channel(CHANNEL_CAPACITY);
                move |relation| gateway.call(relation.to_owned(), app_id, init_payload, Some(to_caller))
                    .and_then( |to_callee_opt|
                        match to_callee_opt {
                            None => Err( ErrorToBeSpecified::TODO( "call was refused be the callee".to_string() ) ),
                            Some(to_callee) => Ok( Call{ sender: to_callee, receiver: from_callee } )
                        }
                    )
            } );

        Box::new(call_fut)
    }
}
