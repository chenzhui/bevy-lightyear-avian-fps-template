package io.github.bevy_lightyear_fps_example.room.service;

/**
 * Raised when a room operation cannot be completed with the current in-memory state.
 */
public class RoomServiceException extends RuntimeException {

    public RoomServiceException(String message) {
        super(message);
    }
}