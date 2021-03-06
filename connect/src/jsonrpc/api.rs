use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

//use failure::Fail;
use futures::{prelude::*, future::Either, sync::{mpsc, oneshot}};
use jsonrpc_core::{Metadata, MetaIoHandler, Params, serde_json as json, types};
use jsonrpc_pubsub::{PubSubHandler, Session as PubSubSession, PubSubMetadata, Subscriber, SubscriptionId};
use tokio_core::reactor;

use mercury_home_protocol::*;
use ::*;
use ::service::*;



pub struct SessionData
{
    pubsub_session: Arc<PubSubSession>,
    dapp_session:   Option<Rc<DAppSession>>,
    cancel_events:  Option<oneshot::Sender<()>>,
}

impl SessionData
{
    pub fn new(transport_tx: mpsc::Sender<String>) -> Self
        { Self{ pubsub_session: Arc::new( PubSubSession::new(transport_tx) ),
                dapp_session: None, cancel_events: None } }
}



#[derive(Clone)]
pub struct Session
{
    inner: Rc<RefCell<SessionData>>
}

impl Session
{
    pub fn new(transport_tx: mpsc::Sender<String>) -> Self
        { Self{ inner: Rc::new( RefCell::new( SessionData::new(transport_tx) ) ) } }

    pub fn dapp_session(&self) -> Option<Rc<DAppSession>>
        { self.inner.borrow().dapp_session.clone() }
    pub fn set_dapp_session(&mut self, dapp_session: Rc<DAppSession>)
        { self.inner.borrow_mut().dapp_session.replace(dapp_session); }

    pub fn take_cancel_events(&mut self) -> Option<oneshot::Sender<()>>
        { self.inner.borrow_mut().cancel_events.take() }
    pub fn set_cancel_events(&mut self, cancel_events: oneshot::Sender<()>)
        { self.inner.borrow_mut().cancel_events.replace(cancel_events); }
}

impl Metadata for Session {}

impl PubSubMetadata for Session {
	fn session(&self) -> Option<Arc<PubSubSession>> {
		Some( self.inner.borrow().pubsub_session.clone() )
	}
}



pub fn create(service: Rc<ConnectService>, handle: reactor::Handle)
    -> Rc< PubSubHandler<Session> >
{
    let mut dispatcher = MetaIoHandler::<Session>::default();

    dispatcher.add_method_with_meta("get_session",
    {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
        struct Request {
            application_id: ApplicationId,
            permissions: Option<DAppPermission>,
        }

        #[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
        struct Response {
            profile_id: String,
        }

        let service = service.clone();
        move |params: Params, mut meta: Session|
        {
            let param_map = match params {
                Params::Map(map) => map,
                Params::None     => return Either::A( Err( types::Error::new(types::ErrorCode::InvalidParams) ).into_future() ),
                Params::Array(_) => return Either::A( Err( types::Error::new(types::ErrorCode::InvalidParams) ).into_future() ),
            };

            let req = match serde_json::from_value::<Request>( json::Value::Object(param_map) ) {
                Ok(req) => req,
                Err(e)  => {
                    debug!("Invalid parameter format: {}", e);
                    return Either::A( Err( types::Error::new(types::ErrorCode::InvalidParams) ).into_future() )
                },
            };

            let resp = service.dapp_session(&req.application_id, req.permissions)
                .map_err( |e| types::Error::new(types::ErrorCode::InternalError) ) // TODO
                .and_then( move |dapp_endpoint| {
                    let resp = Response{ profile_id: dapp_endpoint.selected_profile().into() };
                    meta.set_dapp_session(dapp_endpoint);
                    serde_json::to_value(resp)
                        .map_err( |e| types::Error::new(types::ErrorCode::InternalError) )
                } );
            Either::B(resp)
        }
    } );

    let mut pubsub = PubSubHandler::<Session>::new(dispatcher);
    pubsub.add_subscription( "event",
        ( "subscribe_events", move |params: Params, mut meta: Session, subscriber: Subscriber|
        {
            // TODO set a better ID
            let sink = match subscriber.assign_id( SubscriptionId::String( "TODO_set_a_better_id_here".to_owned() ) )
            {
                Ok(sink) => sink,
                Err(()) => return warn!("Subscription failed"),
            };

            let dapp_session = match meta.dapp_session() {
                Some(s) => s,
                None => return
            };

            let (cancel_tx, cancel_rx) = oneshot::channel();
            meta.set_cancel_events(cancel_tx); // NOTE on a repeated subscribe call this drops the previous tx causing rx to be cancelled

            let fwd_events_fut = dapp_session.checkin()
                .map_err( |e| () ) // TODO
                .and_then( |dapp_events| dapp_events
                    .map( |event| match event {
                        DAppEvent::PairingResponse(resp) => Params::Array( vec![serde_json::Value::String( "Pairing response".into() )] ),
                        DAppEvent::Call(call) => Params::Array( vec![serde_json::Value::String( "Call".into() )] ),
                    } )
                    .forward( sink.sink_map_err( |e| () ) ) // TODO
                )
                .map( |_| () );

            let subscribe_fut = fwd_events_fut.select( cancel_rx.map_err( |_e| () ) ) // TODO
                .map( |((),_pending)| () )
                .map_err( |((),_pending)| () ); // TODO

            handle.spawn( subscribe_fut)
        } ),
        ( "unsubscribe_events", |id: SubscriptionId, mut meta: Session|
        {
            // info!("Cancelling subscription");
            meta.take_cancel_events()
                .ok_or( jsonrpc_core::Error::internal_error() ) // TODO
                .and_then( |cancel| cancel.send( () )
                    .map_err( |_sentitem| jsonrpc_core::Error::internal_error() ) )
                .map( |_| serde_json::Value::Bool(true) )
        }  )
    );

    Rc::new(pubsub)
}
