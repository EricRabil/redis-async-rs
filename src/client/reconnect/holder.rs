/*
 * Copyright 2020 Ben Ashford
 *
 * Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
 * http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
 * <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
 * option. This file may not be copied, modified, or distributed
 * except according to those terms.
 */

use std::{
    future::Future,
    time::{Duration, Instant},
};

use futures_util::{future, TryFutureExt};

use lwactors::{actor, Action, ActorSender};

use crate::error;

use super::ActionWork;

#[derive(Debug)]
pub(crate) struct ConnectionHolder<T, F> {
    queue: ActorSender<
        ConnectionHolderAction<T, F>,
        ConnectionHolderResult<error::Error>,
        error::Error,
    >,
}

impl<T, F> ConnectionHolder<T, F>
where
    T: Send + Sync + 'static,
    F: ActionWork<ConnectionType = T> + Send + 'static,
{
    pub(crate) fn new(t: T) -> Self {
        ConnectionHolder {
            queue: actor(ConnectionHolderState::new(t)),
        }
    }

    pub(crate) fn do_work(&self, f: F) -> impl Future<Output = Result<bool, error::Error>> {
        self.queue
            .invoke(ConnectionHolderAction::DoWork(f))
            .and_then(|result| match result {
                ConnectionHolderResult::DoWork(DoWorkState::Connecting) => future::err(
                    error::Error::Connection(error::ConnectionReason::Connecting),
                ),
                ConnectionHolderResult::DoWork(DoWorkState::NotConnected) => future::ok(false),
                ConnectionHolderResult::DoWork(DoWorkState::ConnectedErr(e)) => future::err(e),
                ConnectionHolderResult::DoWork(DoWorkState::ConnectedOk(())) => future::ok(true),
                _ => panic!("Not a DoWork result"),
            })
    }

    pub(crate) async fn set_connection(&self, con: T) -> Result<(), error::Error> {
        match self
            .queue
            .invoke(ConnectionHolderAction::SetConnection(con))
            .await?
        {
            ConnectionHolderResult::SetConnection => Ok(()),
            _ => panic!("Wrong response"),
        }
    }
}

impl<T, F> Clone for ConnectionHolder<T, F>
where
    T: Send,
{
    fn clone(&self) -> Self {
        ConnectionHolder {
            queue: self.queue.clone(),
        }
    }
}

const MAX_CONNECTION_DUR: Duration = Duration::from_secs(10);

#[derive(Debug)]
enum ConnectionHolderAction<T, F> {
    DoWork(F),
    SetConnection(T),
}

impl<T, F> Action for ConnectionHolderAction<T, F>
where
    T: Send,
    F: ActionWork<ConnectionType = T>,
{
    type State = ConnectionHolderState<T>;
    type Result = ConnectionHolderResult<error::Error>;
    type Error = error::Error;

    fn act(self, state: &mut Self::State) -> Result<Self::Result, Self::Error> {
        let res = match self {
            ConnectionHolderAction::DoWork(work_f) => {
                let dws: DoWorkState<Self::Error> = match state {
                    ConnectionHolderState::Connected(ref con) => match work_f.call(con) {
                        Ok(()) => DoWorkState::ConnectedOk(()),
                        Err(e) => {
                            if e.is_io() || e.is_unexpected() {
                                *state = ConnectionHolderState::Connecting(Instant::now());
                                DoWorkState::NotConnected
                            } else {
                                DoWorkState::ConnectedErr(e)
                            }
                        }
                    },
                    ConnectionHolderState::Connecting(ref mut inst) => {
                        let now = Instant::now();
                        let dur = now - *inst;
                        if dur > MAX_CONNECTION_DUR {
                            *inst = now;
                            DoWorkState::NotConnected
                        } else {
                            DoWorkState::Connecting
                        }
                    }
                };
                ConnectionHolderResult::DoWork(dws)
            }
            ConnectionHolderAction::SetConnection(con) => {
                match state {
                    ConnectionHolderState::Connected(_) => {
                        log::warn!("Cannot set state when in Connected state");
                    }
                    ConnectionHolderState::Connecting(_) => {
                        *state = ConnectionHolderState::Connected(con)
                    }
                }
                ConnectionHolderResult::SetConnection
            }
        };

        Ok(res)
    }
}

#[derive(Debug)]
enum ConnectionHolderState<T>
where
    T: Send,
{
    Connecting(Instant),
    Connected(T),
}

impl<T> ConnectionHolderState<T>
where
    T: Send + 'static,
{
    fn new(t: T) -> Self {
        ConnectionHolderState::Connected(t)
    }
}

#[derive(Debug)]
pub(crate) enum ConnectionHolderResult<E> {
    DoWork(DoWorkState<E>),
    SetConnection,
}

#[derive(Debug)]
pub(crate) enum DoWorkState<E> {
    NotConnected,
    Connecting,
    ConnectedOk(()),
    ConnectedErr(E),
}
