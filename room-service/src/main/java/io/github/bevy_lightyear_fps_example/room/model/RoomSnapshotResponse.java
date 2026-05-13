package io.github.bevy_lightyear_fps_example.room.model;

import java.util.List;

/**
 * Public room state snapshot.
 *
 * @param roomId room id
 * @param currentPlayers number of players in the room
 * @param maxPlayers room capacity
 * @param full whether the room has reached capacity
 * @param members current room members
 */
public record RoomSnapshotResponse(int roomId, int currentPlayers, int maxPlayers, boolean full, List<Long> members) {
}