package io.github.bevy_lightyear_fps_example.room.model;

import java.util.List;

/**
 * Successful room entry response.
 *
 * @param success always true for successful responses
 * @param message human-readable result
 * @param roomId assigned room id
 * @param matchId assigned match id
 * @param userId player id
 * @param serverHost game server host
 * @param serverPort game server port
 * @param entryToken short-lived token validated by the game server
 * @param netcodeClientId Lightyear netcode client id encoded in the connect token
 * @param currentPlayers number of players currently in the room
 * @param maxPlayers room capacity
 * @param members current room members
 */
public record RoomEntryResponse(
        boolean success,
        String message,
        int roomId,
        long matchId,
        long userId,
        String serverHost,
        int serverPort,
        String entryToken,
        long netcodeClientId,
        int currentPlayers,
        int maxPlayers,
        List<Long> members
) {
}