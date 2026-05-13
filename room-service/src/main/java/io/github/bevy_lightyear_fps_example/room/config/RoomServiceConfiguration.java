package io.github.bevy_lightyear_fps_example.room.config;

import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Configuration;

/**
 * Configuration bridge for strongly typed application.yml values.
 */
@Configuration
@EnableConfigurationProperties(GameServerProperties.class)
public class RoomServiceConfiguration {
}