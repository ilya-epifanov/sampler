ARG CROSS_BASE_IMAGE
FROM $CROSS_BASE_IMAGE

ARG CROSS_DEB_ARCH

RUN dpkg --add-architecture $CROSS_DEB_ARCH && \
    apt-get update && \
    apt-get install -y libasound2-dev:$CROSS_DEB_ARCH libasound2:$CROSS_DEB_ARCH
    # libspeechd-dev:$CROSS_DEB_ARCH clang-3.9:$CROSS_DEB_ARCH
