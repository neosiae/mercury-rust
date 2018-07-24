use super::*;
use std;

pub struct Server{
    pub cfg : ServerConfig
}

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {:?}", stringify!($e), e),
    })
}

impl Server{
    pub fn default()->Self{
        Self{
            cfg : ServerConfig::new()
        }
    }

    pub fn new(cfg: ServerConfig)->
    Self{
        Server{
            cfg : cfg
        }
    }

    pub fn run(&self){
        info!("Server mode...");

        match self.cfg.event_timer{
            Some(timer) => Self::event_cycle(timer),
            _ => ()
        }
        match self.cfg.event_file{
            Some(ref file_name)=>Self::handle_event_file(file_name.to_string()),
            _ => ()
        }
        match self.cfg.event_count{
            Some(x) => Self::generate_x(x),
            _ => ()
        }
    }

    pub fn generate_event()->i32{
        info!("Generating event");
        42
    }

    fn event_cycle(event_timer: u64){
        info!("event timer : {}",event_timer);
        std::thread::spawn(move || {
            loop{
                Self::generate_event();
                std::thread::sleep(std::time::Duration::new(event_timer, 0));
            }
        });
    }

    fn generate_x(event_count: u32){
        info!("event count : {}",event_count);
        std::thread::spawn(move || {
            for _i in 0..event_count {
                Self::generate_event();
            }
        });
    }

    fn handle_event_file(file_name: String){
        let mut path = String::from("\0");
        path.push_str(&file_name);
        path.push_str(".sock");
        let sock_path = std::path::PathBuf::from(path);
        let server = t!(UnixListener::bind(&sock_path));
        let uds_incoming = server.incoming()
            .for_each(move | sock| {
                let s : Vec<u8> = Vec::new();
                read(sock, s)
                    .map(|(stream, buf, byte)|{
                        for _ in 0..byte-1{
                            Self::generate_event();
                        }
                    })
                    .then(move |_|future::ok(()))
            }).then(|_| Ok(()));
        tokio::run(uds_incoming);
    }

    pub fn stop_event_generation(){
        info!("Stopped event auto-generation");
    }   
}

impl Future for Server{
    type Item = i32;
    type Error = std::io::Error;
    fn poll(&mut self) -> futures::Poll<Self::Item, Self::Error>{
        self.run();
        Ok(futures::Async::Ready(0))
    }
}