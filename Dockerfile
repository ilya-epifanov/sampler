FROM debian:11-slim
ARG TARGETPLATFORM

ADD artifacts/${TARGETPLATFORM}/sampler /usr/local/bin/sampler
RUN chmod +x /usr/local/bin/sampler

RUN apt-get update && apt-get install -y

RUN mkdir -p /var/local/lib/sampler
WORKDIR "/var/local/lib/sampler"

ENTRYPOINT ["tini", "--", "/usr/local/bin/sampler"]
