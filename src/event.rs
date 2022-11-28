#![allow(non_upper_case_globals)]
use enet_sys::{
    ENetEvent, _ENetEventType_ENET_EVENT_TYPE_CONNECT, _ENetEventType_ENET_EVENT_TYPE_DISCONNECT,
    _ENetEventType_ENET_EVENT_TYPE_NONE, _ENetEventType_ENET_EVENT_TYPE_RECEIVE,
};

use crate::{Host, Packet, Peer, PeerID};

/// This struct represents an event that can occur when servicing a `Host`.
///
/// Note than if an Event is dropped that has a `EventType::Disconnect`, it will
/// mark the Peer as disconnected and drop all data associated with that peer (i.e. `Peer::data`).
/// If you still need that data, make sure to take it out of the peer (e.g. `Peer::take_data`),
/// before dropping the Disconnect Event.
///
/// Also never run `std::mem::forget` on an Event or modify the r#type of the event, as that would
/// skip the cleanup of the Peer.
#[derive(Debug)]
pub struct Event<'a, T> {
    peer: &'a mut Peer<T>,
    peer_id: PeerID,
    r#type: EventType,
}

/// The type of an event.
#[derive(Debug)]
pub enum EventType {
    /// Peer has connected.
    Connect,
    /// Peer has disconnected.
    //
    /// The data of the peer (i.e. `Peer::data`) will be dropped when the received `Event` is dropped.
    Disconnect {
        /// The data associated with this event. Usually a reason for disconnection.
        data: u32,
    },
    /// Peer has received a packet.
    Receive {
        /// ID of the channel that the packet was received on.
        channel_id: u8,
        /// The `Packet` that was received.
        packet: Packet,
    },
}

impl<'a, T> Event<'a, T> {
    pub(crate) fn from_sys_event(event_sys: ENetEvent, host: &'a Host<T>) -> Option<Event<'a, T>> {
        if event_sys.type_ == _ENetEventType_ENET_EVENT_TYPE_NONE {
            return None;
        }

        let peer = unsafe { Peer::new_mut(&mut *event_sys.peer) };
        let peer_id = unsafe { host.peer_id(event_sys.peer) };
        let r#type = match event_sys.type_ {
            _ENetEventType_ENET_EVENT_TYPE_CONNECT => EventType::Connect,
            _ENetEventType_ENET_EVENT_TYPE_DISCONNECT => EventType::Disconnect {
                data: event_sys.data,
            },
            _ENetEventType_ENET_EVENT_TYPE_RECEIVE => EventType::Receive {
                channel_id: event_sys.channelID,
                packet: Packet::from_sys_packet(event_sys.packet),
            },
            _ => panic!("unrecognized event type: {}", event_sys.type_),
        };

        Some(Event {
            peer,
            peer_id,
            r#type,
        })
    }

    /// The peer that this event happened on.
    pub fn peer(&'_ self) -> &'_ Peer<T> {
        &*self.peer
    }

    /// The peer that this event happened on.
    pub fn peer_mut(&'_ mut self) -> &'_ mut Peer<T> {
        self.peer
    }

    /// The `PeerID` of the peer that this event happened on.
    pub fn peer_id(&self) -> PeerID {
        self.peer_id
    }

    /// The type of this event.
    pub fn r#type(&self) -> &EventType {
        &self.r#type
    }

    /// Take the EventType out of this event.
    /// If this peer is a Disconnect event, it will clean up the Peer.
    /// See the `Drop` implementation
    pub fn take_type(mut self) -> EventType {
        // Unfortunately we can't simply take the `r#type` out of the Event, as otherwise the `Drop`
        // implementation would no longer work.
        // We can however, swap the actual EventType with an empty EventType (in this case
        // Connect).
        // As the `Drop` implementation will then do nothing, we need to call cleanup_after_disconnect before we do the swap.
        self.cleanup_after_disconnect();

        let mut r#type = EventType::Connect;
        std::mem::swap(&mut r#type, &mut self.r#type);
        // No need to run the drop implementation.
        std::mem::forget(self);

        r#type
    }

    fn cleanup_after_disconnect(&mut self) {
        match self.r#type {
            EventType::Disconnect { .. } => self.peer.cleanup_after_disconnect(),
            EventType::Connect | EventType::Receive { .. } => {}
        }
    }
}

/// Dropping an `Event` with `EventType::Disconnect` will clean up the Peer, by dropping
/// the data associated with the `Peer`, as well as invalidating the `PeerID`.
impl<'a, T> Drop for Event<'a, T> {
    fn drop(&mut self) {
        self.cleanup_after_disconnect();
    }
}
