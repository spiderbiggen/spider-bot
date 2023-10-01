FROM gcr.io/distroless/cc-debian12 as application

ARG TARGETPLATFORM

COPY "exec/$TARGETPLATFORM" /spider-bot

EXPOSE 8000
ENTRYPOINT ["/spider-bot"]
