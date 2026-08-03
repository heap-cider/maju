import { mkdirSync } from "node:fs";
import { expect, test } from "@playwright/test";

import { waitForAnimations } from "../helpers/animations";
import { installMockBridge } from "../helpers/bridge";

const SCREENSHOT_DIR = "test-results/devices-settings";

async function openDevicesSettings(page: import("@playwright/test").Page) {
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-settings").click();
  await page.getByTestId("profile-popover-settings").click();
  await expect(page.getByTestId("settings-view")).toBeVisible();
  await page.getByTestId("settings-nav-devices").click();
  await expect(page.getByTestId("settings-devices")).toBeVisible();
  await expect(page.getByText("사무실 PC", { exact: true })).toBeVisible();
}

test("device settings shows execution locations and can disconnect another login", async ({
  page,
}) => {
  mkdirSync(SCREENSHOT_DIR, { recursive: true });
  await installMockBridge(page);
  await page.setViewportSize({ height: 760, width: 1280 });
  await openDevicesSettings(page);

  const panel = page.getByTestId("settings-devices");
  await expect(panel.getByText("대표 실행", { exact: false })).toBeVisible();
  await expect(panel.getByText("대기", { exact: false })).toBeVisible();
  await expect(
    panel.getByText("2시간 전 접속", { exact: false }),
  ).toBeVisible();

  await waitForAnimations(page);
  await page.screenshot({
    path: `${SCREENSHOT_DIR}/01-devices-wide-light.png`,
  });

  await panel.getByRole("button", { name: "이름 변경" }).click();
  const nameInput = panel.getByRole("textbox", { name: "기기 이름" });
  await expect(nameInput).toBeFocused();
  await nameInput.fill("작업실 PC");
  await nameInput.press("Enter");
  await expect(panel.getByText("작업실 PC", { exact: true })).toBeVisible();

  await page.setViewportSize({ height: 700, width: 760 });
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(panel.getByText("집 PC", { exact: true })).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await waitForAnimations(page);
  await page.screenshot({
    path: `${SCREENSHOT_DIR}/02-devices-narrow-dark.png`,
  });

  const homeRow = page.getByTestId(
    "device-row-22222222-2222-4222-8222-222222222222",
  );
  await homeRow.getByRole("button", { name: "연결 해제" }).click();
  const dialog = page.getByRole("alertdialog");
  await expect(dialog).toContainText("집 PC 연결을 해제할까요?");
  await dialog.getByRole("button", { name: "연결 해제" }).click();
  await expect(homeRow).toHaveCount(0);
});
