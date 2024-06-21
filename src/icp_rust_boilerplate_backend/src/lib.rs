#[ic_cdk::query]
fn greet(name: String) -> String {
    format!("Hello, {}!", name)
}
#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Ticket {
    id: u64,
    concert_name: String,
    seat_number: String,
    price: f64,
    booking_status: String,
    created_at: u64,
    updated_at: Option<u64>,
}

// a trait that must be implemented for a struct that is stored in a stable struct
impl Storable for Ticket {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

// another trait that must be implemented for a struct that is stored in a stable struct
impl BoundedStorable for Ticket {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static STORAGE: RefCell<StableBTreeMap<u64, Ticket, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct TicketPayload {
    concert_name: String,
    seat_number: String,
    price: f64,
}

#[ic_cdk::query]
fn get_ticket(id: u64) -> Result<Ticket, Error> {
    match _get_ticket(&id) {
        Some(ticket) => Ok(ticket),
        None => Err(Error::NotFound {
            msg: format!("A ticket with id={} not found", id),
        }),
    }
}

#[ic_cdk::update]
fn add_ticket(ticket: TicketPayload) -> Option<Ticket> {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("cannot increment id counter");
    let ticket = Ticket {
        id,
        concert_name: ticket.concert_name,
        seat_number: ticket.seat_number,
        price: ticket.price,
        booking_status: String::from("available"),
        created_at: time(),
        updated_at: None,
    };
    do_insert(&ticket);
    Some(ticket)
}

#[ic_cdk::update]
fn update_ticket(id: u64, payload: TicketPayload) -> Result<Ticket, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut ticket) => {
            ticket.concert_name = payload.concert_name;
            ticket.seat_number = payload.seat_number;
            ticket.price = payload.price;
            ticket.updated_at = Some(time());
            do_insert(&ticket);
            Ok(ticket)
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't update a ticket with id={}. Ticket not found",
                id
            ),
        }),
    }
}

// helper method to perform insert.
fn do_insert(ticket: &Ticket) {
    STORAGE.with(|service| service.borrow_mut().insert(ticket.id, ticket.clone()));
}

#[ic_cdk::update]
fn delete_ticket(id: u64) -> Result<Ticket, Error> {
    match STORAGE.with(|service| service.borrow_mut().remove(&id)) {
        Some(ticket) => Ok(ticket),
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't delete a ticket with id={}. Ticket not found.",
                id
            ),
        }),
    }
}

#[ic_cdk::update]
fn book_ticket(id: u64) -> Result<Ticket, Error> {
    match STORAGE.with(|service| service.borrow().get(&id)) {
        Some(mut ticket) => {
            if ticket.booking_status == "available" {
                ticket.booking_status = String::from("booked");
                ticket.updated_at = Some(time());
                do_insert(&ticket);
                Ok(ticket)
            } else {
                Err(Error::AlreadyBooked {
                    msg: format!("Ticket with id={} is already booked", id),
                })
            }
        }
        None => Err(Error::NotFound {
            msg: format!(
                "Couldn't book a ticket with id={}. Ticket not found",
                id
            ),
        }),
    }
}

#[derive(candid::CandidType, Deserialize, Serialize)]
enum Error {
    NotFound { msg: String },
    AlreadyBooked { msg: String },
}

// a helper method to get a ticket by id. used in get_ticket/update_ticket
fn _get_ticket(id: &u64) -> Option<Ticket> {
    STORAGE.with(|service| service.borrow().get(id))
}

// need this to generate candid
ic_cdk::export_candid!();
