FROM rustembedded/cross:armv7-unknown-linux-gnueabihf-0.2.1

RUN apt-get update
RUN apt-get install --assume-yes multistrap symlinks
COPY multistrap-config /
RUN multistrap -a armhf -f multistrap-config /build/armhf-sysroot/
RUN rm multistrap-config

COPY relink.sh /
RUN cd /build/armhf-sysroot && /relink.sh

# Link this single header across for a real complicated set of reasons.
# This is necessary for OpenSSL but breaks everything else.
# We need a way to do this *just* for the OpenSSL config file, but *not* for
# everything else (because I *think* the compiler version in use on the host
# arch is different from the libraries in use on the target, so if we do it for everything else, we'll get rando breakages).
#ENV C_INCLUDE_PATH=/build/armhf-sysroot/usr/include/arm-linux-gnueabihf
RUN ln -s /build/armhf-sysroot/usr/include/arm-linux-gnueabihf/openssl/opensslconf.h \
          /build/armhf-sysroot/usr/include/openssl/

# Here's the reference value, taken from `pkg-config --variable pc_path pkg-config`
# See https://askubuntu.com/questions/210210/pkg-config-path-environment-variable
# Then replace any arch-specific values with the new architecture.
# Note that PKG_CONFIG_LIBDIR overrides the default search paths, see https://www.freedesktop.org/wiki/Software/pkg-config/CrossCompileProposal/
# Reference value: /usr/local/lib/x86_64-linux-gnu/pkgconfig:/usr/local/lib/pkgconfig:/usr/local/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/lib/pkgconfig:/usr/share/pkgconfig
# TODO
ENV PKG_CONFIG_SYSROOT_DIR=/build/armhf-sysroot
ENV PKG_CONFIG_LIBDIR=/build/armhf-sysroot/usr/lib/arm-linux-gnueabihf/pkgconfig
