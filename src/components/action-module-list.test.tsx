/**
 * @vitest-environment jsdom
 */
import { describe, it, expect } from 'vitest';
import { render, fireEvent } from '@testing-library/preact';
import { ActionModuleList } from './action-module-list';

const SAMPLE_MODULES = [
  { id: 'json', label: 'JSON 查看', desc: '识别 JSON 格式内容', trigger: '内容以 { 或 [ 开头' },
  { id: 'folder', label: '打开路径', desc: '识别 Windows 文件路径', trigger: 'Windows 路径格式' },
  { id: 'math', label: '计算器', desc: '识别数学表达式', trigger: '数字和运算符' },
];

describe('ActionModuleList (display-only)', () => {
  it('renders all modules', () => {
    const { container } = render(<ActionModuleList modules={SAMPLE_MODULES} />);
    const items = container.querySelectorAll('.action-module-item');
    expect(items.length).toBe(3);
  });

  it('displays module label and description', () => {
    const { getByText } = render(<ActionModuleList modules={SAMPLE_MODULES} />);
    expect(getByText('JSON 查看')).toBeTruthy();
    expect(getByText('识别 JSON 格式内容')).toBeTruthy();
    expect(getByText('打开路径')).toBeTruthy();
    expect(getByText('识别 Windows 文件路径')).toBeTruthy();
  });

  it('has no toggle switches or move buttons', () => {
    const { container } = render(<ActionModuleList modules={SAMPLE_MODULES} />);
    expect(container.querySelectorAll('.toggle-switch').length).toBe(0);
    expect(container.querySelectorAll('.action-module-move').length).toBe(0);
  });

  it('shows expand buttons for each module', () => {
    const { container } = render(<ActionModuleList modules={SAMPLE_MODULES} />);
    const expandBtns = container.querySelectorAll('.action-module-expand');
    expect(expandBtns.length).toBe(3);
  });

  it('expands a module on click to show trigger detail', () => {
    const { container, getAllByText, getByText } = render(<ActionModuleList modules={SAMPLE_MODULES} />);
    const expandBtns = container.querySelectorAll('.action-module-expand');

    fireEvent.click(expandBtns[0]);
    // "desc" text appears in both the collapsed desc and the expanded detail
    expect(getAllByText('识别 JSON 格式内容').length).toBeGreaterThanOrEqual(1);
    // trigger text only appears when expanded
    expect(getByText('内容以 { 或 [ 开头')).toBeTruthy();
  });

  it('collapse on second click', () => {
    const { container, queryByText } = render(<ActionModuleList modules={SAMPLE_MODULES} />);
    const expandBtns = container.querySelectorAll('.action-module-expand');

    // Expand
    fireEvent.click(expandBtns[0]);
    expect(queryByText('内容以 { 或 [ 开头')).toBeTruthy();

    // Collapse
    fireEvent.click(expandBtns[0]);
    expect(queryByText('内容以 { 或 [ 开头')).toBeNull();
  });
});
