package io.github.bevy_lightyear_fps_example.room.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

/**
 * Game server connection and room allocation settings.
 *
 * @param host game server host returned to clients
 * @param port game server port returned to clients
 * @param roomCount fixed number of in-memory rooms managed by this sample service
 * @param roomCapacity maximum players allowed in a room
 * @param tokenTtlSeconds entry token lifetime in seconds
 */
@ConfigurationProperties(prefix = "game.server")
public record GameServerProperties(String host, int port, int roomCount, int roomCapacity, long tokenTtlSeconds) {

    public GameServerProperties {
        if (roomCount < 1) {
            throw new IllegalArgumentException("game.server.room-count must be at least 1");
        }
        if (roomCapacity < 1) {
            throw new IllegalArgumentException("game.server.room-capacity must be at least 1");
        }
        if (tokenTtlSeconds < 1) {
            throw new IllegalArgumentException("game.server.token-ttl-seconds must be at least 1");
        }
    }
}