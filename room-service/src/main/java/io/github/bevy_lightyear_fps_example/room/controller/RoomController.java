package io.github.bevy_lightyear_fps_example.room.controller;

import io.github.bevy_lightyear_fps_example.room.model.JoinRoomRequest;
import io.github.bevy_lightyear_fps_example.room.model.RoomEntryResponse;
import io.github.bevy_lightyear_fps_example.room.model.RoomSnapshotResponse;
import io.github.bevy_lightyear_fps_example.room.model.ValidateTokenRequest;
import io.github.bevy_lightyear_fps_example.room.model.ValidatedTokenResponse;
import io.github.bevy_lightyear_fps_example.room.service.RoomService;
import jakarta.validation.Valid;
import java.util.List;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.DeleteMapping;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Minimal room API used by the open source Bevy client/server example.
 */
@RestController
@RequestMapping("/api/rooms")
public class RoomController {

    private final RoomService roomService;

    public RoomController(RoomService roomService) {
        this.roomService = roomService;
    }

    /**
     * Enters the requested room, or the first room with free capacity when roomId is omitted.
     *
     * @param request room entry request containing the player id and optional target room id
     * @return connection information and a short-lived entry token
     */
    @PostMapping("/join")
    public ResponseEntity<RoomEntryResponse> joinRoom(@Valid @RequestBody JoinRoomRequest request) {
        return ResponseEntity.ok(roomService.joinRoom(request));
    }

    /**
     * Lists all managed rooms and their current members.
     *
     * @return room snapshots
     */
    @GetMapping
    public ResponseEntity<List<RoomSnapshotResponse>> listRooms() {
        return ResponseEntity.ok(roomService.listRooms());
    }

    /**
     * Returns one room snapshot.
     *
     * @param roomId target room id
     * @return room snapshot
     */
    @GetMapping("/{roomId}")
    public ResponseEntity<RoomSnapshotResponse> getRoom(@PathVariable int roomId) {
        return ResponseEntity.ok(roomService.getRoom(roomId));
    }

    /**
     * Removes a player from a room.
     *
     * @param roomId room id
     * @param userId player id
     * @return updated room snapshot
     */
    @DeleteMapping("/{roomId}/players/{userId}")
    public ResponseEntity<RoomSnapshotResponse> leaveRoom(@PathVariable int roomId, @PathVariable long userId) {
        return ResponseEntity.ok(roomService.leaveRoom(roomId, userId));
    }

    /**
     * Validates a room entry token before the game server accepts the player.
     *
     * @param request token validation request
     * @return token payload when valid
     */
    @PostMapping("/validate")
    public ResponseEntity<ValidatedTokenResponse> validateToken(@Valid @RequestBody ValidateTokenRequest request) {
        return ResponseEntity.ok(roomService.validateToken(request));
    }
}