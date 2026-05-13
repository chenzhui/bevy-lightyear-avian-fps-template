package io.github.bevy_lightyear_fps_example.room.service;

import io.github.bevy_lightyear_fps_example.room.config.GameServerProperties;
import io.github.bevy_lightyear_fps_example.room.model.JoinRoomRequest;
import io.github.bevy_lightyear_fps_example.room.model.RoomEntryResponse;
import io.github.bevy_lightyear_fps_example.room.model.RoomSnapshotResponse;
import io.github.bevy_lightyear_fps_example.room.model.ValidateTokenRequest;
import io.github.bevy_lightyear_fps_example.room.model.ValidatedTokenResponse;
import java.nio.charset.StandardCharsets;
import java.time.Clock;
import java.time.Instant;
import java.util.Base64;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.ConcurrentHashMap;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.stereotype.Service;

/**
 * In-memory room service for the public example.
 *
 * This intentionally avoids Redis and database dependencies so the open source sample can run locally with only Java.
 */
@Service
public class RoomService {

    private final Clock clock;
    private final GameServerProperties properties;
    private final Map<Integer, Room> rooms;
    private final Map<String, EntryToken> tokens = new ConcurrentHashMap<>();

    @Autowired
    public RoomService(GameServerProperties properties) {
        this(properties, Clock.systemUTC());
    }

    RoomService(GameServerProperties properties, Clock clock) {
        this.clock = clock;
        this.properties = properties;
        this.rooms = new ConcurrentHashMap<>();
        for (int roomId = 1; roomId <= properties.roomCount(); roomId++) {
            rooms.put(roomId, new Room(roomId, properties.roomCapacity()));
        }
    }

    /**
     * Adds a user to a room and returns game server connection data.
     *
     * @param request join request
     * @return entry response with token
     */
    public synchronized RoomEntryResponse joinRoom(JoinRoomRequest request) {
        long userId = request.userId();
        Room room = selectRoom(request.roomId());
        if (!room.contains(userId) && room.isFull()) {
            throw new RoomServiceException("Room " + room.id() + " is full");
        }

        room.add(userId);
        long matchId = room.id();
        long netcodeClientId = userId;
        String token = issueToken(room.id(), matchId, userId, netcodeClientId);
        return new RoomEntryResponse(
                true,
                "Joined room",
                room.id(),
                matchId,
                userId,
                properties.host(),
                properties.port(),
                token,
                netcodeClientId,
                room.currentPlayers(),
                room.capacity(),
                room.membersSnapshot()
        );
    }

    /**
     * Lists room snapshots ordered by room id.
     *
     * @return all rooms
     */
    public List<RoomSnapshotResponse> listRooms() {
        return rooms.values().stream()
                .sorted(Comparator.comparingInt(Room::id))
                .map(this::toSnapshot)
                .toList();
    }

    /**
     * Returns one room snapshot.
     *
     * @param roomId room id
     * @return room snapshot
     */
    public RoomSnapshotResponse getRoom(int roomId) {
        return toSnapshot(requiredRoom(roomId));
    }

    /**
     * Removes a user from a room.
     *
     * @param roomId room id
     * @param userId player id
     * @return updated room snapshot
     */
    public synchronized RoomSnapshotResponse leaveRoom(int roomId, long userId) {
        Room room = requiredRoom(roomId);
        room.remove(userId);
        tokens.entrySet().removeIf(entry -> entry.getValue().roomId() == roomId && entry.getValue().userId() == userId);
        return toSnapshot(room);
    }

    /**
     * Validates a previously issued room entry token.
     *
     * @param request validation request
     * @return validation result
     */
    public ValidatedTokenResponse validateToken(ValidateTokenRequest request) {
        EntryToken token = tokens.get(request.token());
        if (token == null || token.expiresAt().isBefore(Instant.now(clock))) {
            tokens.remove(request.token());
            return new ValidatedTokenResponse(false, "Invalid or expired token", null, null, null, null);
        }
        Room room = rooms.get(token.roomId());
        if (room == null || !room.contains(token.userId())) {
            return new ValidatedTokenResponse(false, "Player is no longer in the room", null, null, null, null);
        }
        return new ValidatedTokenResponse(true, "Token valid", token.userId(), token.matchId(), token.roomId(), token.netcodeClientId());
    }

    private Room selectRoom(Integer requestedRoomId) {
        if (requestedRoomId != null) {
            return requiredRoom(requestedRoomId);
        }
        Optional<Room> room = rooms.values().stream()
                .filter(candidate -> !candidate.isFull())
                .min(Comparator.comparingInt(Room::id));
        return room.orElseThrow(() -> new RoomServiceException("No available room"));
    }

    private Room requiredRoom(int roomId) {
        Room room = rooms.get(roomId);
        if (room == null) {
            throw new RoomNotFoundException(roomId);
        }
        return room;
    }

    private String issueToken(int roomId, long matchId, long userId, long netcodeClientId) {
        Instant expiresAt = Instant.now(clock).plusSeconds(properties.tokenTtlSeconds());
        String rawToken = roomId + ":" + matchId + ":" + userId + ":" + netcodeClientId + ":" + expiresAt.toEpochMilli() + ":" + System.nanoTime();
        String token = Base64.getUrlEncoder().withoutPadding().encodeToString(rawToken.getBytes(StandardCharsets.UTF_8));
        tokens.put(token, new EntryToken(roomId, matchId, userId, netcodeClientId, expiresAt));
        return token;
    }

    private RoomSnapshotResponse toSnapshot(Room room) {
        return new RoomSnapshotResponse(room.id(), room.currentPlayers(), room.capacity(), room.isFull(), room.membersSnapshot());
    }

    private record EntryToken(int roomId, long matchId, long userId, long netcodeClientId, Instant expiresAt) {
    }
}