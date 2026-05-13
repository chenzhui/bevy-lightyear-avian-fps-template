package io.github.bevy_lightyear_fps_example.room.controller;

import io.github.bevy_lightyear_fps_example.room.model.ValidateTokenRequest;
import io.github.bevy_lightyear_fps_example.room.model.ValidatedTokenResponse;
import io.github.bevy_lightyear_fps_example.room.service.RoomService;
import jakarta.validation.Valid;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.annotation.PostMapping;
import org.springframework.web.bind.annotation.RequestBody;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

/**
 * Compatibility API used by the game server during match admission.
 */
@RestController
@RequestMapping("/api/match")
public class MatchController {

    private final RoomService roomService;

    public MatchController(RoomService roomService) {
        this.roomService = roomService;
    }

    @PostMapping("/validate")
    public ResponseEntity<ValidatedTokenResponse> validateToken(@Valid @RequestBody ValidateTokenRequest request) {
        return ResponseEntity.ok(roomService.validateToken(request));
    }
}