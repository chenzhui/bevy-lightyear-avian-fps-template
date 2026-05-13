package io.github.bevy_lightyear_fps_example.room.controller;

import io.github.bevy_lightyear_fps_example.room.service.RoomNotFoundException;
import io.github.bevy_lightyear_fps_example.room.service.RoomServiceException;
import java.util.Map;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.web.bind.MethodArgumentNotValidException;
import org.springframework.web.bind.annotation.ExceptionHandler;
import org.springframework.web.bind.annotation.RestControllerAdvice;

/**
 * Maps service and validation errors to compact JSON responses.
 */
@RestControllerAdvice
public class RoomExceptionHandler {

    @ExceptionHandler(RoomNotFoundException.class)
    public ResponseEntity<Map<String, Object>> handleRoomNotFound(RoomNotFoundException ex) {
        return ResponseEntity.status(HttpStatus.NOT_FOUND).body(error(ex.getMessage()));
    }

    @ExceptionHandler(RoomServiceException.class)
    public ResponseEntity<Map<String, Object>> handleRoomService(RoomServiceException ex) {
        return ResponseEntity.badRequest().body(error(ex.getMessage()));
    }

    @ExceptionHandler(MethodArgumentNotValidException.class)
    public ResponseEntity<Map<String, Object>> handleValidation(MethodArgumentNotValidException ex) {
        String message = ex.getBindingResult().getFieldErrors().stream()
                .findFirst()
                .map(error -> error.getField() + " " + error.getDefaultMessage())
                .orElse("Request body is invalid");
        return ResponseEntity.badRequest().body(error(message));
    }

    private Map<String, Object> error(String message) {
        return Map.of("success", false, "message", message);
    }
}