package io.github.bevy_lightyear_fps_example.room.model;

import jakarta.validation.constraints.NotBlank;

/**
 * Request for validating a room entry token.
 *
 * @param token token returned by /api/rooms/join
 */
public record ValidateTokenRequest(@NotBlank String token) {
}