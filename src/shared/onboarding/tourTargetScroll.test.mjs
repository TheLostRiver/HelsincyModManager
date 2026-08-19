import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { cwd } from "node:process";
import { test } from "node:test";

const repoRoot = cwd();

function readProjectFile(relativePath) {
  return readFileSync(join(repoRoot, relativePath), "utf8");
}

/**
 * 取出 measure() 里"元素身份变化"分支的函数体，用于断言滚动挂在这个分支上。
 */
function targetIdentityBranch(source) {
  const branch = /if \(target !== currentTarget\) \{([\s\S]*?)\n {6}\}/.exec(source);
  assert.ok(branch, "measure() 应保留 target !== currentTarget 的身份变化分支");
  return branch[1];
}

test("引导滚动由目标身份变化驱动，而非 effect 顶层的一次性检查", () => {
  const source = readProjectFile("src/shared/onboarding/useTourTarget.ts");

  // 缺陷背景：路由层进场动画 route-layer-enter 的 from { opacity: 0 } 在
  // animation-fill-mode: both 下让动画启动前的计算 opacity 就是 0，
  // isUsableTourTarget 因此判定目标不可用。引导跨路由推进步骤恰好落在这一两帧，
  // 顶层一次性检查拿到 null 就永久跳过滚动，目标停在上一页遗留的滚动位置。
  assert.match(
    targetIdentityBranch(source),
    /scrollTargetIntoSafeViewport\(target\)/,
    "滚动必须在目标身份变化时触发，才能复用 pollAnimatedTarget 的重试",
  );

  // 顶层不得再出现一次性的 initialTarget 解析。
  assert.doesNotMatch(
    source,
    /const initialTarget\s*=/,
    "不应保留 effect 顶层的一次性目标解析",
  );
});

test("引导滚动复用既有的动画轮询生命周期，不新增计时器", () => {
  const source = readProjectFile("src/shared/onboarding/useTourTarget.ts");

  // 轮询是滚动能在进场动画结束后补上的唯一依赖。这三项若被改动，
  // 滚动会退回到"动画期间解析失败即放弃"的旧行为。
  assert.match(source, /TOUR_TARGET_ANIMATION_POLL_MS = 1_200/);
  assert.match(source, /window\.requestAnimationFrame\(pollAnimatedTarget\)/);
  assert.match(source, /timestamp < animationPollDeadline && currentTarget === null/);

  // 滚动不得引入独立的 setTimeout / setInterval 重试。
  const scrollHelper = /function scrollTargetIntoSafeViewport\([\s\S]*?\n\}/.exec(source);
  assert.ok(scrollHelper, "应有 scrollTargetIntoSafeViewport 辅助函数");
  assert.doesNotMatch(scrollHelper[0], /setTimeout|setInterval|requestAnimationFrame/);
});

test("引导滚动保持居中且尊重减少动态效果设置", () => {
  const source = readProjectFile("src/shared/onboarding/useTourTarget.ts");
  const scrollHelper = /function scrollTargetIntoSafeViewport\([\s\S]*?\n\}/.exec(source)[0];

  // block: "center" 是必需的：高亮矩形会被 expandAndClampRect 钳到视口范围内，
  // 目标居中才能保证整块画得出来。用 "nearest" 会让贴边目标继续被裁。
  assert.match(scrollHelper, /block:\s*"center"/);
  assert.match(scrollHelper, /inline:\s*"nearest"/);
  assert.match(scrollHelper, /prefersReducedMotion\(\)\s*\?\s*"auto"\s*:\s*"smooth"/);

  // 只在目标贴边或越界时才滚，否则每次重新测量都会抢走用户自己的滚动位置。
  assert.match(scrollHelper, /if \(!isOutsideSafeViewport\) return;/);
});

test("引导不直接改写滚动容器位置", () => {
  const source = readProjectFile("src/shared/onboarding/useTourTarget.ts");

  // 直接写 scrollTop 会绕过 scroll-behavior 与 prefers-reduced-motion，
  // 也会和 .app-surface 上其他滚动逻辑（如 Mod 库回到顶部）打架。
  assert.doesNotMatch(source, /scrollTop\s*=/);
  assert.doesNotMatch(source, /scrollTo\(/);
});

test("引导滚动不会与用户滚动争夺滚动条", () => {
  const source = readProjectFile("src/shared/onboarding/useTourTarget.ts");

  // 用户手动滚动只触发 measure，不改变元素身份，因此不会重新滚动。
  // 这条依赖"滚动只写在身份变化分支内"——measure 主体里不得有第二处滚动调用。
  const scrollCalls = source.match(/scrollTargetIntoSafeViewport\(/g) ?? [];
  assert.equal(
    scrollCalls.length,
    2,
    "应只有一处定义与一处调用；多余的调用点会让每次重新测量都可能抢滚动条",
  );
});
