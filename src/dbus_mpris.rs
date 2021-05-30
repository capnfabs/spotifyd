use chrono::prelude::*;
use futures::{Future, Stream, StreamExt};
use futures::channel::oneshot;
use futures;
use librespot::{
    connect::spirc::Spirc,
    core::{
        keymaster::{get_token, Token as LibrespotToken},
        mercury::MercuryError,
        session::Session,
    },
};
use log::{info, warn};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rspotify::spotify::{
    client::Spotify, model::offset::for_position, oauth2::TokenInfo as RspotifyToken, senum::*,
    util::datetime_to_timestamp,
};
use std::{collections::HashMap, env, rc::Rc, thread};
use futures::task::{Context, Poll};
use std::pin::Pin;
use dbus_tokio::connection;
use dbus_crossroads::{Crossroads, IfaceBuilder};
use librespot::playback::player::PlayerEvent;
use dbus::message::MatchRule;
use dbus::channel::MatchingReceiver;

//, mut player_event_channel: Pin<Box<dyn Stream<Item=PlayerEvent>>>
pub async fn dbus_server_2(session: Session, spirc: Rc<Spirc>, device_name: String) -> Result<(), Box<dyn std::error::Error>> {
    let (resource, connection) = connection::new_session_sync()?;
    tokio::spawn(async {
        let err = resource.await;
        panic!("Lost connection to D-Bus: {}", err);
    });
        connection.request_name(
            "org.mpris.MediaPlayer2.spotifyd",
            false,
            true,
            true
        ).await?;

    // BUILD CROSSROADS
    let mut cr = Crossroads::new();
    cr.set_async_support(Some((connection.clone(), Box::new(|x| { tokio::spawn(x); }))));
    // https://specifications.freedesktop.org/mpris-spec/latest/Media_Player.html
    let mediaplayer2_iface = cr.register("org.mpris.MediaPlayer2", |b| {
        b.method("Raise", (), (), |_, _, ():()| Ok(()));
        b.method_with_cr("Quit", (), (),  |mut ctx, cr, ():()| {
            //let local_spirc = spirc.clone();
            //local_spirc.shutdown();
            Ok(())
        });
        b.property("CanRaise")
            .emits_changed_const()
            .get(|_, _| Ok(false));
        b.property("CanQuit")
            .emits_changed_const()
            .get(|_, _| Ok(true));
        b.property("HasTrackList")
            .emits_changed_const()
            .get(|_, _| Ok(false));
        b.property("Identity")
            .emits_changed_const()
            .get(|_, _| Ok("Spotifyd".to_owned()));
        b.property("SupportedUriSchemes")
            .emits_changed_const()
            .get(|_, _| Ok(vec!["spotify".to_owned()]));
        b.property::<Vec<String>,_>("SupportedMimeTypes")
            .emits_changed_const()
            .get(|_, _| Ok(vec![]));
    });

    let player_iface = cr.register("org.mpris.MediaPlayer2.Player", |b: &mut IfaceBuilder<PlayerData>|{
        b.property("PlaybackStatus")
            .emits_changed_false()
            .get_with_cr(|ctx, cr| {
                let data: &mut PlayerData = cr.data_mut(ctx.path()).unwrap();
                Ok(data.playback_status.to_str().to_owned())
            });

        // Optional:
        // b.property("LoopStatus");
        // b.property("Shuffle");

        // b.property("Metadata")
        //     .emits_changed_false()
        //     .get_with_cr(|ctx, cr| {
        //         // TODO
        //         Ok(false)
        //     })
        // ;
        // b.property("Volume");
        // b.property("Position");

        b.property("MinimumRate")
            .emits_changed_false()
            .get(|_,_| Ok(1.0));
        b.property("MaximumRate")
            .emits_changed_false()
            .get(|_,_| Ok(1.0));

        // TODO: the answer to this should depend on current playback state.
        b.property("CanGoNext")
            .emits_changed_false()
            .get(|_,_| Ok(true));
        b.property("CanGoPrevious")
            .emits_changed_false()
            .get(|_,_| Ok(true));
        b.property("CanPlay")
            .emits_changed_false()
            .get(|_,_| Ok(true));
        b.property("CanPause")
            .emits_changed_false()
            .get(|_,_| Ok(true));

        // For now, don't support this
        b.property("CanSeek").emits_changed_const().get(|_,_| Ok(false));
        b.property("CanControl")
            .emits_changed_const()
            .get(|_,_| Ok(true));

    });

    cr.insert("/", &[mediaplayer2_iface, player_iface], PlayerData {  playback_status: MprisPlaybackStatus::Paused });

    connection.start_receive(MatchRule::new_method_call(), Box::new(move |msg, conn| {
        cr.handle_message(msg, conn).unwrap();
        true
    }));
    //
    // // TODO here:
    // // - Figure out how to share state between threads
    // //   (maybe with the approach here? https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_adv_server_cr.rs?)
    // // - Figure out how to trigger changed notifications when state has changed
    // // - Use the command line runner from the librespot example for clues as to how this state
    // //     machine should work.
    //
    // let playerdata: &mut PlayerData = cr.data_mut("org.mpris.MediaPlayer2.Player".into()).unwrap();
    //
    // while let Some(event) = player_event_channel.as_mut().next().await {
    //     match event {
    //         PlayerEvent::Stopped { .. } => {
    //             playerdata.playback_status = MprisPlaybackStatus::Stopped;
    //         }
    //         PlayerEvent::Started { .. } => {}
    //         PlayerEvent::Changed { .. } => {}
    //         PlayerEvent::Loading { .. } => {}
    //         PlayerEvent::Preloading { .. } => {}
    //         PlayerEvent::Playing { .. } => {
    //             playerdata.playback_status = MprisPlaybackStatus::Playing;
    //         }
    //         PlayerEvent::Paused { .. } => {
    //             playerdata.playback_status = MprisPlaybackStatus::Paused;
    //         }
    //         PlayerEvent::TimeToPreloadNextTrack { .. } => {}
    //         PlayerEvent::EndOfTrack { .. } => {}
    //         PlayerEvent::Unavailable { .. } => {}
    //         PlayerEvent::VolumeSet { .. } => {}
    //     }
    // }

    // Run forever
    futures::future::pending::<()>().await;
    unreachable!()
}

enum MprisPlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl MprisPlaybackStatus {
    fn to_str(&self) -> &'static str {
        match self {
            MprisPlaybackStatus::Playing => "Playing",
            MprisPlaybackStatus::Paused => "Paused",
            MprisPlaybackStatus::Stopped => "Stopped",
        }
    }
}

struct PlayerData {
    playback_status: MprisPlaybackStatus
}
