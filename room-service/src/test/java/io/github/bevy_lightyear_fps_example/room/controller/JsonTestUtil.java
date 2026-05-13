package io.github.bevy_lightyear_fps_example.room.controller;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

/**
 * Small JSON helper used by MVC tests.
 */
final class JsonTestUtil {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    private JsonTestUtil() {
    }

    static String stringValue(String json, String fieldName) throws Exception {
        JsonNode node = MAPPER.readTree(json).get(fieldName);
        if (node == null || !node.isTextual()) {
            throw new IllegalArgumentException("Missing string field: " + fieldName);
        }
        return node.asText();
    }
}