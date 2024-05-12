# Start with a rust alpine image
FROM rust:1-alpine3.19
# This is important, see https://github.com/rust-lang/docker-rust/issues/85
ENV RUSTFLAGS="-C target-feature=-crt-static"
# if needed, add additional dependencies here
RUN apk add --no-cache musl-dev protoc
# set the workdir and copy the source into it
WORKDIR /app
COPY ./ /app
# do a release build
RUN cargo build --release
RUN strip target/release/my-plugin

# use a plain alpine image, the alpine version needs to match the builder
FROM alpine:3.19
RUN addgroup -S hypi && adduser -S hypi -G hypi
# if needed, install additional dependencies here
RUN apk add --no-cache libgcc
# copy the binary into the final image
COPY --from=0 /app/target/release/my-plugin /home/hypi/rapid-plugin
RUN chown hypi:hypi /home/hypi/rapid-plugin && chmod +x /home/hypi/rapid-plugin
USER hypi
# set the binary as entrypoint
ENTRYPOINT ["/home/hypi/rapid-plugin"]
#CMD ["/home/hypi/rapid-plugin"]
