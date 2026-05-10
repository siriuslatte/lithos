import { Children, Fragment, ReactElement, ReactNode, isValidElement } from 'react';
import { Tab, Tabs } from 'nextra-theme-docs';

type ConfigFormatTabProps = {
  label?: string;
  children: ReactNode;
};

export function ConfigFormatTab({ children }: ConfigFormatTabProps) {
  return <Fragment>{children}</Fragment>;
}

export function ConfigFormatTabs({ children }: { children: ReactNode }) {
  const items = Children.toArray(children).filter(
    (child): child is ReactElement<ConfigFormatTabProps> => isValidElement(child)
  );

  if (items.length === 0) {
    return null;
  }

  if (items.length === 1) {
    const [firstItem] = items;
    if (!firstItem) {
      return null;
    }

    return <Fragment>{firstItem.props.children}</Fragment>;
  }

  return (
    <Tabs items={items.map((item) => item.props.label ?? 'Example')}>
      {items.map((item, index) => (
        <Tab key={index}>{item.props.children}</Tab>
      ))}
    </Tabs>
  );
}