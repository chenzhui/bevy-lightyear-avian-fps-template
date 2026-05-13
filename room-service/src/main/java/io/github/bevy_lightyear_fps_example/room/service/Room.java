package io.github.bevy_lightyear_fps_example.room.service;

import java.util.ArrayList;
import java.util.List;

/**
 * Mutable in-memory room state guarded by RoomService synchronization.
 */
final class Room {

    private final int id;
    private final int capacity;
    private final List<Long> members = new ArrayList<>();

    Room(int id, int capacity) {
        this.id = id;
        this.capacity = capacity;
    }

    int id() {
        return id;
    }

    int capacity() {
        return capacity;
    }

    int currentPlayers() {
        return members.size();
    }

    boolean isFull() {
        return members.size() >= capacity;
    }

    boolean contains(long userId) {
        return members.contains(userId);
    }

    void add(long userId) {
        if (!contains(userId)) {
            members.add(userId);
        }
    }

    boolean remove(long userId) {
        return members.remove(userId);
    }

    List<Long> membersSnapshot() {
        return List.copyOf(members);
    }
}