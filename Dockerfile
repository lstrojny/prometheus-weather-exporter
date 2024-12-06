FROM alpine:3.21
EXPOSE 36333
ARG TARGETPLATFORM
ARG CONTAINER_BINARY_DIR
COPY ${CONTAINER_BINARY_DIR}/${TARGETPLATFORM}/prometheus-weathermen /usr/local/bin/
RUN mkdir -p /etc/prometheus-weathermen
COPY weathermen.toml.dist /etc/prometheus-weathermen
ENV PROMW_HTTP__ADDRESS=0.0.0.0
ENTRYPOINT ["/usr/local/bin/prometheus-weathermen"]
