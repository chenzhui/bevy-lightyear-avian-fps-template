package io.github.bevy_lightyear_fps_example.room.service;

/**
 * Raised when the requested room id is outside the configured room range.
 */
public class RoomNotFoundException extends RuntimeException {

    public RoomNotFoundException(int roomId) {
        super("Room " + roomId + " does not exist");
    }
}