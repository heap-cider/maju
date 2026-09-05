import Foundation
import Testing

@testable import MajuPushKit

@Test func `Round-trip opaque navigation target through notification user info`() {
  let target = MajuPushNavigationTarget(
    eventID: "MESSAGE-ID",
    communityID: "community-id",
    channelID: "CHANNEL/GENERAL"
  )

  #expect(
    MajuPushNavigationTarget.decodeIfPresent(
      from: [MajuPushNavigationTarget.userInfoKey: target.userInfoValue]
    ) == target
  )
  #expect(target.eventID == "MESSAGE-ID")
  #expect(target.channelID == "CHANNEL/GENERAL")
}

@Test func `Reject incomplete or malformed navigation target`() {
  #expect(
    MajuPushNavigationTarget.decodeIfPresent(
      from: [
        MajuPushNavigationTarget.userInfoKey: [
          "event_id": "message-id",
          "community_id": "community-id",
        ]
      ]
    ) == nil
  )

  #expect(
    MajuPushNavigationTarget.decodeIfPresent(
      from: [
        MajuPushNavigationTarget.userInfoKey: [
          "event_id": "",
          "community_id": "community-id",
          "channel_id": "channel-id",
        ]
      ]
    ) == nil
  )

  #expect(
    MajuPushNavigationTarget.decodeIfPresent(
      from: [
        MajuPushNavigationTarget.userInfoKey: [
          "event_id": "message-id",
          "community_id": "community-id",
          "channel_id": "",
        ]
      ]
    ) == nil
  )
}

@Test func `Buffer preserves cold-start target until consumed`() {
  let first = MajuPushNavigationTarget(
    eventID: String(repeating: "a", count: 64),
    communityID: "community-id",
    channelID: "123e4567-e89b-42d3-a456-426614174000"
  )
  let second = MajuPushNavigationTarget(
    eventID: String(repeating: "b", count: 64),
    communityID: "community-id",
    channelID: "123e4567-e89b-42d3-a456-426614174000"
  )
  let buffer = MajuPushNavigationBuffer()

  buffer.record(first)
  buffer.remove(ifMatching: second)
  #expect(buffer.peek() == first)
  #expect(buffer.take() == first)
  #expect(buffer.take() == nil)
}
