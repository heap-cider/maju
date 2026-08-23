import { expect, test } from "@playwright/test";

import { waitForAnimations } from "../helpers/animations";
import { installMockBridge } from "../helpers/bridge";

test("an empty Projects page opens the real create flow", async ({ page }) => {
  await page.addInitScript(() => {
    window.localStorage.setItem(
      "maju-feature-overrides-v1",
      JSON.stringify({ projects: true }),
    );
    window.localStorage.setItem("maju-e2e-empty-projects", "1");
  });
  await installMockBridge(page);
  await page.setViewportSize({ height: 720, width: 1100 });
  await page.goto("/", { waitUntil: "domcontentloaded" });
  await page.getByTestId("open-projects-view").click();

  const emptyState = page.getByTestId("projects-empty-state");
  await expect(emptyState).toBeVisible();
  await expect(emptyState.getByText("No projects yet")).toBeVisible();
  await expect(
    emptyState.getByRole("button", { name: "Create project" }),
  ).toBeVisible();

  await waitForAnimations(page);
  await page.screenshot({
    path: "test-results/projects-empty-state/01-empty-projects.png",
  });

  await page.setViewportSize({ height: 640, width: 760 });
  await expect(
    emptyState.getByRole("button", { name: "Create project" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await waitForAnimations(page);
  await page.screenshot({
    path: "test-results/projects-empty-state/02-empty-projects-narrow.png",
  });

  await page.setViewportSize({ height: 720, width: 1100 });

  await emptyState.getByRole("button", { name: "Create project" }).click();
  const dialog = page.getByTestId("create-project-dialog");
  await expect(dialog).toBeVisible();
  await expect(page.getByTestId("create-project-name")).toBeFocused();
  await expect(
    page.getByTestId("create-project-access-channel"),
  ).not.toHaveValue("");
  await expect(page.getByTestId("create-project-description")).toBeVisible();
  await expect(page.getByTestId("create-project-clone-url")).toBeVisible();
  await expect(page.getByTestId("create-project-web-url")).toBeVisible();

  await waitForAnimations(page);
  await dialog.screenshot({
    path: "test-results/projects-empty-state/03-create-project.png",
  });

  await page.getByTestId("create-project-name").fill("first-project");
  await page
    .getByTestId("create-project-description")
    .fill("Created from the empty Projects page");
  await page
    .getByTestId("create-project-clone-url")
    .fill("https://github.com/heap-cider/maju.git");
  await page
    .getByTestId("create-project-web-url")
    .fill("https://github.com/heap-cider/maju");
  await page.getByTestId("create-project-submit").click();
  await expect(dialog).toBeHidden();
  await expect(page.getByTestId("project-grid-card-name")).toHaveText(
    "first-project",
  );
});
