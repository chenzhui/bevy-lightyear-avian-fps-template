package io.github.bevy_lightyear_fps_example.room.model;

import jakarta.validation.constraints.NotNull;
import jakarta.validation.constraints.Positive;

/**
 * Request for entering a game room.
 *
 * @param userId player id assigned by the caller
 * @param roomId optional fixed room id; null means auto-assign
 */
public record JoinRoomRequest(@NotNull @Positive Long userId, @Positive Integer roomId) {
}