package io.github.bevy_lightyear_fps_example.room.model;

/**
 * Token validation result.
 *
 * @param success whether the token is currently valid
 * @param message human-readable result
 * @param userId player id encoded in the token, null when invalid
 * @param matchId match id encoded in the token, null when invalid
 * @param roomId room id encoded in the token, null when invalid
 * @param netcodeClientId Lightyear netcode client id encoded in the token, null when invalid
 */
public record ValidatedTokenResponse(boolean success, String message, Long userId, Long matchId, Integer roomId, Long netcodeClientId) {
}