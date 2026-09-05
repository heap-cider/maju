import MajuPushKit
import Foundation
import Security

/// Keychain-backed endpoint grant storage. The opaque grant is never written to
/// UserDefaults or logs. Dart can read the closed record through the push bridge.
final class MajuPushEndpointGrantKeychainStore: MajuPushEndpointGrantStore {
  private static let service = "maju.push.endpoint-grants"
  private static let recordsAccount = "v1"
  private static let pendingAccount = "pending-v1"

  private let accessGroup: String?

  init(accessGroup: String?) {
    self.accessGroup = accessGroup
  }

  func records() throws -> [MajuPushEndpointGrantRecord] {
    var query = baseQuery(account: Self.recordsAccount)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return [] }
    guard status == errSecSuccess, let data = result as? Data else {
      throw keychainError(status, operation: "read")
    }
    do {
      return try JSONDecoder().decode([MajuPushEndpointGrantRecord].self, from: data)
    } catch {
      throw NSError(
        domain: "MajuPushEndpointGrantStore",
        code: 1,
        userInfo: [NSLocalizedDescriptionKey: "Stored endpoint grants are invalid: \(error)"]
      )
    }
  }

  func save(_ record: MajuPushEndpointGrantRecord) throws {
    var all = try records()
    all.removeAll {
      $0.relayOrigin == record.relayOrigin && $0.appProfile == record.appProfile
    }
    all.append(record)
    try replace(all, account: Self.recordsAccount)
  }

  func pendingEnrollment(
    relayOrigin: String,
    appProfile: String
  ) throws -> MajuPushPendingEnrollmentRecord? {
    try pendingEnrollments().first {
      $0.relayOrigin == relayOrigin && $0.appProfile == appProfile
    }
  }

  func savePendingEnrollment(_ record: MajuPushPendingEnrollmentRecord) throws {
    var all = try pendingEnrollments()
    all.removeAll {
      $0.relayOrigin == record.relayOrigin && $0.appProfile == record.appProfile
    }
    all.append(record)
    try replace(all, account: Self.pendingAccount)
  }

  func removePendingEnrollment(relayOrigin: String, appProfile: String) throws {
    var all = try pendingEnrollments()
    all.removeAll {
      $0.relayOrigin == relayOrigin && $0.appProfile == appProfile
    }
    try replace(all, account: Self.pendingAccount)
  }

  private func pendingEnrollments() throws -> [MajuPushPendingEnrollmentRecord] {
    var query = baseQuery(account: Self.pendingAccount)
    query[kSecReturnData as String] = true
    query[kSecMatchLimit as String] = kSecMatchLimitOne
    var result: CFTypeRef?
    let status = SecItemCopyMatching(query as CFDictionary, &result)
    if status == errSecItemNotFound { return [] }
    guard status == errSecSuccess, let data = result as? Data else {
      throw keychainError(status, operation: "read pending enrollment")
    }
    do {
      return try JSONDecoder().decode([MajuPushPendingEnrollmentRecord].self, from: data)
    } catch {
      throw NSError(
        domain: "MajuPushEndpointGrantStore",
        code: 2,
        userInfo: [NSLocalizedDescriptionKey: "Stored pending enrollments are invalid: \(error)"]
      )
    }
  }

  private func replace<T: Encodable>(_ values: [T], account: String) throws {
    let data = try JSONEncoder().encode(values)
    let updateStatus = SecItemUpdate(
      baseQuery(account: account) as CFDictionary,
      [kSecValueData as String: data] as CFDictionary
    )
    if updateStatus == errSecSuccess { return }
    guard updateStatus == errSecItemNotFound else {
      throw keychainError(updateStatus, operation: "update")
    }

    var add = baseQuery(account: account)
    add[kSecValueData as String] = data
    add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
    let addStatus = SecItemAdd(add as CFDictionary, nil)
    guard addStatus == errSecSuccess else {
      throw keychainError(addStatus, operation: "add")
    }
  }

  private func baseQuery(account: String) -> [String: Any] {
    var query: [String: Any] = [
      kSecClass as String: kSecClassGenericPassword,
      kSecAttrService as String: Self.service,
      kSecAttrAccount as String: account,
    ]
    if let accessGroup, !accessGroup.isEmpty {
      query[kSecAttrAccessGroup as String] = accessGroup
    }
    return query
  }

  private func keychainError(_ status: OSStatus, operation: String) -> Error {
    NSError(
      domain: NSOSStatusErrorDomain,
      code: Int(status),
      userInfo: [
        NSLocalizedDescriptionKey:
          "Endpoint grant Keychain \(operation) failed: \(SecCopyErrorMessageString(status, nil) ?? "unknown" as CFString)"
      ]
    )
  }
}

extension MajuPushEndpointGrantRecord {
  var flutterArguments: [String: Any] {
    let arguments: [String: Any] = [
      "relayOrigin": relayOrigin,
      "relayPubkey": relayPubkey,
      "installationId": installationId,
      "endpointGrant": endpointGrant,
      "endpointHash": endpointHash,
      "appProfile": appProfile,
      "endpointEpoch": endpointEpoch,
      "generation": generation,
      "expiresAt": expiresAt,
    ]
    return arguments
  }
}
