package io.github.bevy_lightyear_fps_example.room.controller;

import static org.hamcrest.Matchers.hasSize;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.delete;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.get;
import static org.springframework.test.web.servlet.request.MockMvcRequestBuilders.post;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.jsonPath;
import static org.springframework.test.web.servlet.result.MockMvcResultMatchers.status;

import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.autoconfigure.web.servlet.AutoConfigureMockMvc;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.test.annotation.DirtiesContext;
import org.springframework.http.MediaType;
import org.springframework.test.context.TestPropertySource;
import org.springframework.test.web.servlet.MockMvc;
import org.springframework.test.web.servlet.MvcResult;

/**
 * API-level regression tests for the room entry flow.
 */
@SpringBootTest
@AutoConfigureMockMvc
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
@TestPropertySource(properties = {
        "game.server.host=127.0.0.1",
        "game.server.port=5888",
        "game.server.room-count=2",
        "game.server.room-capacity=2",
        "game.server.token-ttl-seconds=300"
})
class RoomControllerTest {

    @Autowired
    private MockMvc mvc;

    @Test
    void joinRoomShouldAssignFirstAvailableRoom() throws Exception {
        mvc.perform(post("/api/rooms/join")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":1001}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.success").value(true))
                .andExpect(jsonPath("$.roomId").value(1))
                .andExpect(jsonPath("$.matchId").value(1))
                .andExpect(jsonPath("$.netcodeClientId").value(1001))
                .andExpect(jsonPath("$.serverHost").value("127.0.0.1"))
                .andExpect(jsonPath("$.serverPort").value(5888))
                .andExpect(jsonPath("$.currentPlayers").value(1))
                .andExpect(jsonPath("$.members[0]").value(1001));
    }

    @Test
    void validateTokenShouldReturnTokenPayload() throws Exception {
        MvcResult joined = mvc.perform(post("/api/rooms/join")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":2001,\"roomId\":2}"))
                .andExpect(status().isOk())
                .andReturn();

        String token = JsonTestUtil.stringValue(joined.getResponse().getContentAsString(), "entryToken");
        mvc.perform(post("/api/rooms/validate")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"token\":\"" + token + "\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.success").value(true))
                .andExpect(jsonPath("$.userId").value(2001))
                .andExpect(jsonPath("$.matchId").value(2))
                .andExpect(jsonPath("$.roomId").value(2))
                .andExpect(jsonPath("$.netcodeClientId").value(2001));
    }

    @Test
    void matchValidateShouldReturnTokenPayload() throws Exception {
        MvcResult joined = mvc.perform(post("/api/rooms/join")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":2002,\"roomId\":2}"))
                .andExpect(status().isOk())
                .andReturn();

        String token = JsonTestUtil.stringValue(joined.getResponse().getContentAsString(), "entryToken");
        mvc.perform(post("/api/match/validate")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"token\":\"" + token + "\"}"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.success").value(true))
                .andExpect(jsonPath("$.userId").value(2002))
                .andExpect(jsonPath("$.matchId").value(2))
                .andExpect(jsonPath("$.roomId").value(2))
                .andExpect(jsonPath("$.netcodeClientId").value(2002));
    }

    @Test
    void fullRoomShouldRejectNewPlayer() throws Exception {
        mvc.perform(post("/api/rooms/join").contentType(MediaType.APPLICATION_JSON).content("{\"userId\":3001,\"roomId\":1}"))
                .andExpect(status().isOk());
        mvc.perform(post("/api/rooms/join").contentType(MediaType.APPLICATION_JSON).content("{\"userId\":3002,\"roomId\":1}"))
                .andExpect(status().isOk());

        mvc.perform(post("/api/rooms/join")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":3003,\"roomId\":1}"))
                .andExpect(status().isBadRequest())
                .andExpect(jsonPath("$.success").value(false));
    }

    @Test
    void leaveRoomShouldRemovePlayer() throws Exception {
        mvc.perform(post("/api/rooms/join")
                        .contentType(MediaType.APPLICATION_JSON)
                        .content("{\"userId\":4001,\"roomId\":2}"))
                .andExpect(status().isOk());

        mvc.perform(delete("/api/rooms/2/players/4001"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.currentPlayers").value(0))
                .andExpect(jsonPath("$.members", hasSize(0)));

        mvc.perform(get("/api/rooms/2"))
                .andExpect(status().isOk())
                .andExpect(jsonPath("$.currentPlayers").value(0));
    }
}