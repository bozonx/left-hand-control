import assert from 'node:assert/strict'
import { $, browser } from '@wdio/globals'

export async function byTestId(testId) {
  return $(`[data-testid="${testId}"]`)
}

export async function waitForTestId(testId, timeout = 30000) {
  const element = await byTestId(testId)
  await element.waitForExist({ timeout })
  return element
}

export async function completeWelcomeIfPresent() {
  const welcomeEmpty = await byTestId('welcome-empty-layout')
  const exists = await welcomeEmpty.waitForExist({
    timeout: 1500,
    reverse: false,
  }).catch(() => false)
  if (!exists) return

  await welcomeEmpty.click()
  await waitForTestId('settings-page', 30000)
}

export async function openHome() {
  await completeWelcomeIfPresent()
  const home = await waitForTestId('home-nav-button')
  await home.click()
  await waitForTestId('layouts-page')
}

export async function openSettings() {
  await completeWelcomeIfPresent()
  const settings = await waitForTestId('settings-nav-button')
  await settings.click()
  await waitForTestId('settings-page')
}

export async function waitForAttribute(testId, name, predicate, timeout = 30000) {
  const element = await waitForTestId(testId)
  await browser.waitUntil(
    async () => predicate(await element.getAttribute(name)),
    {
      timeout,
      timeoutMsg: `Timed out waiting for ${testId} ${name}`,
    },
  )
  return element.getAttribute(name)
}

export function normalizePathForAssert(value) {
  return value.replaceAll('\\', '/')
}

export function assertContainsPath(actual, expected) {
  assert.ok(
    normalizePathForAssert(actual).includes(normalizePathForAssert(expected)),
    `Expected "${actual}" to contain "${expected}"`,
  )
}
