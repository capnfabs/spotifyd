FROM rustembedded/cross:armv7-unknown-linux-gnueabihf-0.2.1

# Install debootstrap
RUN apt-get update && apt-get install multistrap --assume-yes
# Install the target system
RUN mkdir /sysroot-armhf
# We'll explain this soon
COPY multistrap-config /
RUN multistrap -a armhf -f multistrap-config /sysroot-armhf


#RUN apt-get update && apt-get install symlinks --assume-yes
#COPY relink.sh /
#RUN cd /sysroot-armhf && /relink.sh

RUN ln -s /sysroot-armhf/usr/include/arm-linux-gnueabihf/openssl/opensslconf.h \
          /sysroot-armhf/usr/include/openssl/

ENV PKG_CONFIG_LIBDIR_armv7_unknown_linux_gnueabihf=/sysroot-armhf/usr/lib/arm-linux-gnueabihf/pkgconfig
ENV PKG_CONFIG_SYSROOT_DIR_armv7_unknown_linux_gnueabihf=/sysroot-armhf
ENV CARGO_TARGET_ARMV7_UNKNOWN_LINUX_GNUEABIHF_RUSTFLAGS="-C link-arg=--sysroot=/sysroot-armhf"
