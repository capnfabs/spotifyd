use std::rc::Rc;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::{Crossroads, IfaceBuilder};
use dbus_tokio::connection;
use futures::{Stream, StreamExt};
use futures;
use log::{warn, info};
use librespot::{
    connect::spirc::Spirc,
    core::{
        session::Session,
    },
};
use librespot::playback::player::PlayerEvent;
use librespot::metadata::{Track, Metadata};
use std::collections::HashMap;
use dbus::arg::{Variant, RefArg};
use dbus::MethodErr;
use librespot::connect::spirc::ContextChangedEvent;

pub async fn dbus_server_2(
    session: Session,
    spirc: Rc<Spirc>,
    device_name: String,
    mut player_event_channel: Pin<Box<dyn Stream<Item=PlayerEvent>>>,
    mut context_event_channel: Pin<Box<dyn Stream<Item=ContextChangedEvent>>>) -> Result<(), Box<dyn std::error::Error>> {
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
        b.method_with_cr("Quit", (), (),  |_, _, ():()| {
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

        b.property("Metadata")
            .emits_changed_false()
            .get_with_cr(|ctx, cr| {
                let data: &mut PlayerData = cr.data_mut(ctx.path()).unwrap();
                let context_uri = &data.context_state;
                data.current_track.as_ref().map(|x| {
                        let mut data = x.to_xesam();
                        if let Some(state) = context_uri {
                            data.insert("spotifyd:contextUri".to_string(), Variant(Box::new(state.context_uri.clone())));
                            data.insert("spotifyd:playlistTrackNumber".to_string(), Variant(Box::new(state.playing_track_index + 1)));
                        }
                        data
                    }).ok_or(MethodErr::failed("womp"))
            });

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

    cr.insert("/", &[mediaplayer2_iface, player_iface], PlayerData { playback_status: MprisPlaybackStatus::Paused, current_track: None, context_state: None });

    // The Arc<Mutex<_>> thing here is a pattern for sharing state across thread contexts etc so that we can update the data
    // stored in Crossroads' system. Examples:
    // - DBUS: https://github.com/diwic/dbus-rs/blob/master/dbus-tokio/examples/tokio_adv_server_cr.rs
    // - Rust Book: https://doc.rust-lang.org/book/ch20-02-multithreaded.html
    let cr_arc = Arc::new(Mutex::new(cr));

    let cr_arc_copy = cr_arc.clone();
    connection.start_receive(MatchRule::new_method_call(), Box::new( move |msg, conn| {
        cr_arc_copy.lock().unwrap().handle_message(msg, conn).unwrap();
        true
    }));

    // TODO here:
    // - Figure out how to trigger changed notifications when state has changed
    // - Use the command line runner from the librespot example for clues as to how this state
    //     machine should work.

    loop {
        let mut pe = player_event_channel.as_mut();
        let mut ce = context_event_channel.as_mut();
        tokio::select! {
            event = pe.next() => {
                info!("Got player event: {:?}", event);
                if let Some(event) = event {
                    let mut cr = cr_arc.lock().unwrap();
                    let playerdata: &mut PlayerData = cr.data_mut(&"/".into()).unwrap();
                    match event {
                        PlayerEvent::Stopped { .. } => {
                            playerdata.playback_status = MprisPlaybackStatus::Stopped;
                        }
                        PlayerEvent::Started { .. } => {}
                        PlayerEvent::Changed { .. } => {}
                        PlayerEvent::Loading { .. } => {}
                        PlayerEvent::Preloading { .. } => {}
                        PlayerEvent::Playing { track_id, .. } => {
                            playerdata.current_track = Track::get(&session, track_id).await.map_or_else(
                                |err| {
                                    warn!("Couldn't load metadata for track: {:?}", err);
                                    None
                                },
                                |metadata| Some(TrackMetadata::from_librespot(metadata)),
                            );
                            playerdata.playback_status = MprisPlaybackStatus::Playing;
                        }
                        PlayerEvent::Paused { .. } => {
                            playerdata.playback_status = MprisPlaybackStatus::Paused;
                        }
                        PlayerEvent::TimeToPreloadNextTrack { .. } => {}
                        PlayerEvent::EndOfTrack { .. } => {}
                        PlayerEvent::Unavailable { .. } => {}
                        PlayerEvent::VolumeSet { .. } => {}
                    }
                }
            },
            event = ce.next() => {
                info!("Got context changed event: {:?}", event);
                if let Some(event) = event {
                    let mut cr = cr_arc.lock().unwrap();
                    let playerdata: &mut PlayerData = cr.data_mut(&"/".into()).unwrap();
                    playerdata.context_state = Some(event);
                }
            },
        }
    }
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

struct TrackMetadata {
    track: Track,
}

impl TrackMetadata {
    fn from_librespot(t: Track) -> TrackMetadata {
        TrackMetadata {
            track: t
        }
    }

    fn to_xesam(&self) -> HashMap<String, Variant<Box<dyn RefArg>>> {
        let t = &self.track;
        let uri = t.id.to_uri();
        let album = t.album.name.clone();
        let album_artist: Vec<_> = (&t.album.artists).iter().map(|artist| artist.name.clone()).collect();
        let artist: Vec<_> = t.artists.iter().map(|artist| artist.name.clone()).collect();
        let title = t.name.clone();

        let mut xesam: HashMap<String, Variant<Box<dyn RefArg>>> = HashMap::new();
        xesam.insert("mpris:trackid".to_string(), Variant(Box::new(uri.clone())));
        // millis -> micros
        xesam.insert("mpris:length".to_string(), Variant(Box::new(t.duration * 1000)));
        // Complex to route
        //xesam.insert("mpris:artUrl".to_string(), Variant(Box::new(None)));
        xesam.insert("xesam:title".to_string(), Variant(Box::new(title)));
        xesam.insert("xesam:album".to_string(), Variant(Box::new(album)));
        xesam.insert("xesam:artist".to_string(), Variant(Box::new(artist)));
        xesam.insert("xesam:albumArtist".to_string(), Variant(Box::new(album_artist)));
        xesam.insert("xesam:autoRating".to_string(), Variant(Box::new(f64::from(t.popularity) / 100.0)));
        xesam.insert("xesam:trackNumber".to_string(), Variant(Box::new(t.track_number)));
        xesam.insert("xesam:discNumber".to_string(), Variant(Box::new(t.disc_number)));
        xesam.insert("xesam:url".to_string(), Variant(Box::new(uri)));

        xesam
    }
}

struct PlayerData {
    playback_status: MprisPlaybackStatus,
    current_track: Option<TrackMetadata>,
    context_state: Option<ContextChangedEvent>,
}
