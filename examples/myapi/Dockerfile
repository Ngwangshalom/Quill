# Builds Quill, then runs server.ql. Hosting services like Render, Railway
# and Fly.io read this file, build it, and set PORT for you.

FROM rust:1-slim AS build
# Quill's web module isn't in a release yet, so build it from the repository.
RUN cargo install --git https://github.com/Ngwangshalom/Quill --branch main --locked

FROM debian:bookworm-slim
COPY --from=build /usr/local/cargo/bin/quill /usr/local/bin/quill
WORKDIR /app
COPY . .
ENV QUILL_NO_EDITOR_SETUP=1
CMD ["quill", "run", "server.ql"]
