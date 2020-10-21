FROM rustembedded/cross:armv7-unknown-linux-gnueabihf-0.2.1

#RUN dpkg --add-architecture armhf
# TODO: should this instead use the RPI repo?
#RUN echo "deb [arch=armhf] http://raspbian.raspberrypi.org/raspbian/ buster main" | tee -a /etc/apt/sources.list
#RUN apt-key adv --fetch-keys http://raspbian.raspberrypi.org/raspbian.public.key

RUN apt-get update
RUN apt-get install --assume-yes multistrap symlinks
COPY multistrap-config /
RUN multistrap -a armhf -f multistrap-config /build/armhf-sysroot/
RUN rm multistrap-config

COPY relink.sh /
RUN cd /build/armhf-sysroot && /relink.sh

# Here's the reference value, taken from `pkg-config --variable pc_path pkg-config`
# See https://askubuntu.com/questions/210210/pkg-config-path-environment-variable
# Then replace any arch-specific values with the new architecture.
# Note that PKG_CONFIG_LIBDIR overrides the default search paths, see https://www.freedesktop.org/wiki/Software/pkg-config/CrossCompileProposal/
# Reference value: /usr/local/lib/x86_64-linux-gnu/pkgconfig:/usr/local/lib/pkgconfig:/usr/local/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig
# TODO
ENV PKG_CONFIG_SYSROOT_DIR=/build/armhf-sysroot
ENV PKG_CONFIG_LIBDIR=/build/armhf-sysroot/usr/lib/arm-linux-gnueabihf/pkgconfig
# This is necessary for OpenSSL but breaks everything else.
# We need a way to do this *just* for the OpenSSL config file, but *not* for
# everything else (because I *think* the compiler version in use on the host
# arch is different from the libraries in use on the target, so if we do it for everything else, we'll get rando breakages).
ENV CPATH=/build/armhf-sysroot/usr/include/arm-linux-gnueabihf

# TODO
# echo -e '[target.arm-unknown-linux-gnueabihf.dbus]\nrustc-link-lib = ["dbus-1", "gcrypt", "gpg-error", "lz4", "lzma", "systemd"]' | sudo tee -a /.cargo/config

#[General]
#arch=armhf
#directory=/build/armhf-sysroot/
#aptsources=Raspbian
#
#[Raspbian]
#packages=
#source=http://raspbian.raspberrypi.org/raspbian/
#suite=buster
