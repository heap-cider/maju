import MajuPushKit
import Intents
import UserNotifications
import XCTest

@testable import Maju

final class MajuCommunicationNotificationTests: XCTestCase {
  func testIntentUsesVerifiedSenderAvatarAndGroupName() throws {
    let resolution = communicationResolution(
      displayName: "Alice",
      groupName: "General",
      avatarPNG: Data([0x89, 0x50, 0x4E, 0x47])
    )
    let descriptor = try XCTUnwrap(
      MajuCommunicationNotificationDescriptor(resolution: resolution)
    )

    let intent = MajuCommunicationNotificationPresenter.makeIntent(descriptor)

    XCTAssertEqual(intent.sender?.displayName, "Alice")
    XCTAssertEqual(intent.sender?.customIdentifier, descriptor.senderIdentifier)
    XCTAssertNotNil(intent.sender?.image)
    XCTAssertNotNil(intent.image(forParameterNamed: \.speakableGroupName))
    XCTAssertEqual(intent.content, "Hello Maju")
    XCTAssertEqual(intent.speakableGroupName?.spokenPhrase, "General")
    XCTAssertEqual(intent.conversationIdentifier, resolution.conversationIdentifier)
    XCTAssertNil(intent.recipients)
    XCTAssertEqual(descriptor.recipientCount, 1)
    XCTAssertEqual(
      (intent.donationMetadata as? INSendMessageIntentDonationMetadata)?.recipientCount,
      1
    )
  }

  func testDirectMessageRetainsSenderAvatarWithoutGroupMetadata() throws {
    let resolution = communicationResolution(
      displayName: "Alice",
      groupName: nil,
      avatarPNG: Data([0x89, 0x50, 0x4E, 0x47])
    )
    let descriptor = try XCTUnwrap(
      MajuCommunicationNotificationDescriptor(resolution: resolution)
    )

    let intent = MajuCommunicationNotificationPresenter.makeIntent(descriptor)

    XCTAssertEqual(intent.sender?.displayName, "Alice")
    XCTAssertNotNil(intent.sender?.image)
    XCTAssertNil(intent.image(forParameterNamed: \.speakableGroupName))
    XCTAssertNil(intent.speakableGroupName)
    XCTAssertNil(intent.donationMetadata)
  }

  func testMissingVerifiedRecipientCountUsesOrdinaryPresentation() {
    XCTAssertNil(
      MajuCommunicationNotificationDescriptor(
        resolution: communicationResolution(recipientCount: nil)
      )
    )
  }

  func testPresentationFallsBackWhenDonationFails() {
    let ordinary = UNMutableNotificationContent()
    ordinary.title = "Alice"
    ordinary.body = "Hello Maju"
    var updateCalled = false
    let presenter = MajuCommunicationNotificationPresenter(
      donate: { _, completion in
        completion(NSError(domain: "test", code: 1))
      },
      updateContent: { content, _ in
        updateCalled = true
        return content
      }
    )
    let completed = expectation(description: "ordinary fallback returned")

    presenter.present(
      ordinaryContent: ordinary,
      resolution: communicationResolution()
    ) { content in
      XCTAssertEqual(content.title, "Alice")
      XCTAssertEqual(content.body, "Hello Maju")
      completed.fulfill()
    }

    wait(for: [completed], timeout: 1)
    XCTAssertFalse(updateCalled)
  }

  func testPresentationDonatesBeforeSpecializing() {
    let ordinary = UNMutableNotificationContent()
    ordinary.title = "Alice"
    var order: [String] = []
    let presenter = MajuCommunicationNotificationPresenter(
      donate: { _, completion in
        order.append("donate")
        completion(nil)
      },
      updateContent: { _, _ in
        order.append("update")
        let specialized = UNMutableNotificationContent()
        specialized.title = "specialized"
        return specialized
      }
    )
    let completed = expectation(description: "specialized content returned")

    presenter.present(
      ordinaryContent: ordinary,
      resolution: communicationResolution()
    ) { content in
      XCTAssertEqual(content.title, "specialized")
      completed.fulfill()
    }

    wait(for: [completed], timeout: 1)
    XCTAssertEqual(order, ["donate", "update"])
  }

  func testPresentationFallsBackWhenSpecializationFails() {
    let ordinary = UNMutableNotificationContent()
    ordinary.title = "Alice"
    ordinary.body = "Hello Maju"
    let presenter = MajuCommunicationNotificationPresenter(
      donate: { _, completion in completion(nil) },
      updateContent: { _, _ in throw NSError(domain: "test", code: 2) }
    )
    let completed = expectation(description: "ordinary fallback returned")

    presenter.present(
      ordinaryContent: ordinary,
      resolution: communicationResolution()
    ) { content in
      XCTAssertEqual(content.title, "Alice")
      XCTAssertEqual(content.body, "Hello Maju")
      completed.fulfill()
    }

    wait(for: [completed], timeout: 1)
  }

  private func communicationResolution(
    displayName: String = "Alice",
    groupName: String? = "General",
    avatarPNG: Data? = nil,
    recipientCount: Int? = 1
  ) -> MajuPushResolution {
    let communityID = "community-id"
    let channelID = "channel/general:v5"
    return MajuPushResolution(
      title: displayName,
      body: "Hello Maju",
      subtitle: "Community",
      threadIdentifier: MajuPushPresentationIdentity.conversation(
        communityID: communityID,
        channelID: channelID
      ),
      navigationTarget: MajuPushNavigationTarget(
        eventID: "message-id",
        communityID: communityID,
        channelID: channelID
      ),
      senderPubkey: String(repeating: "a", count: 64),
      senderAvatarPNG: avatarPNG,
      conversationIdentifier: MajuPushPresentationIdentity.conversation(
        communityID: communityID,
        channelID: channelID
      ),
      conversationDisplayName: groupName,
      conversationRecipientCount: recipientCount
    )
  }
}

final class MajuPushSnapshotEnrichmentTests: XCTestCase {
  func testMetadataAuthorityUsesCurrentAppProfileForMatchingRelay() {
    let correctProfile = grant(
      appProfile: MajuDevPushEnrollmentDriver.appProfile,
      generation: 2,
      metadataPubkey: String(repeating: "a", count: 64)
    )
    let wrongProfile = grant(
      appProfile: "other-profile",
      generation: 99,
      metadataPubkey: String(repeating: "b", count: 64)
    )

    XCTAssertEqual(
      MajuPushSnapshotBridge.relayMetadataPubkey(
        relayURL: "wss://relay.example/",
        grants: [wrongProfile, correctProfile]
      ),
      correctProfile.relayMetadataPubkey
    )
  }

  private func grant(
    appProfile: String,
    generation: Int64,
    metadataPubkey: String
  ) -> MajuPushEndpointGrantRecord {
    MajuPushEndpointGrantRecord(
      relayOrigin: "https://relay.example",
      relayPubkey: String(repeating: "c", count: 64),
      relayMetadataPubkey: metadataPubkey,
      installationId: "installation",
      endpointGrant: "opaque-grant",
      endpointHash: String(repeating: "d", count: 64),
      appProfile: appProfile,
      endpointEpoch: 1,
      generation: generation,
      expiresAt: 1_900_000_000
    )
  }
}

final class MajuPushNotificationResponseTests: XCTestCase {
  func testValidDefaultActionRoutesAndCompletesExactlyOnce() {
    let target = MajuPushNavigationTarget(
      eventID: "message-id",
      communityID: "community-id",
      channelID: "opaque-channel-id"
    )
    var routedTargets: [MajuPushNavigationTarget] = []
    var forwarded = 0
    var completions = 0

    MajuPushNotificationResponseCoordinator.handle(
      actionIdentifier: UNNotificationDefaultActionIdentifier,
      userInfo: [MajuPushNavigationTarget.userInfoKey: target.userInfoValue],
      onTarget: { routedTargets.append($0) },
      forwardToFlutter: { pluginCompletion in
        forwarded += 1
        pluginCompletion()
        pluginCompletion()
      },
      completion: { completions += 1 }
    )

    XCTAssertEqual(routedTargets, [target])
    XCTAssertEqual(forwarded, 1)
    XCTAssertEqual(completions, 1)
  }

  func testMalformedTargetFallsBackToOneCompletion() {
    var routedTargets: [MajuPushNavigationTarget] = []
    var completions = 0

    MajuPushNotificationResponseCoordinator.handle(
      actionIdentifier: UNNotificationDefaultActionIdentifier,
      userInfo: [MajuPushNavigationTarget.userInfoKey: ["event_id": ""]],
      onTarget: { routedTargets.append($0) },
      forwardToFlutter: { _ in },
      completion: { completions += 1 }
    )

    XCTAssertTrue(routedTargets.isEmpty)
    XCTAssertEqual(completions, 1)
  }

  func testNonDefaultActionIgnoresLateDuplicatePluginCompletion() throws {
    var routedTargets: [MajuPushNavigationTarget] = []
    var pluginCompletion: (() -> Void)?
    var completions = 0

    MajuPushNotificationResponseCoordinator.handle(
      actionIdentifier: "maju.reply",
      userInfo: [:],
      onTarget: { routedTargets.append($0) },
      forwardToFlutter: { pluginCompletion = $0 },
      completion: { completions += 1 }
    )
    let capturedCompletion = try XCTUnwrap(pluginCompletion)
    capturedCompletion()
    capturedCompletion()

    XCTAssertTrue(routedTargets.isEmpty)
    XCTAssertEqual(completions, 1)
  }
}
