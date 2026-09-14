import type { Meta, StoryObj } from "@storybook/react-vite";
import { LumenLoader, IconLoader } from "@opal/components";
import { SvgSettings } from "@opal/icons";

const meta: Meta<typeof LumenLoader> = {
  title: "opal/components/Loader",
  component: LumenLoader,
  tags: ["autodocs"],
};

export default meta;
type Story = StoryObj;

// LumenLoader: the branded octagon/logo crossfade.

export const LumenMark: Story = {
  render: () => <LumenLoader />,
};

export const LumenSizes: Story = {
  render: () => (
    <div className="flex items-end gap-6">
      <LumenLoader size={24} />
      <LumenLoader size={40} />
      <LumenLoader size={64} />
    </div>
  ),
};

export const LumenColors: Story = {
  render: () => (
    <div className="flex items-end gap-6">
      <LumenLoader />
      <LumenLoader color="text-04" />
      <LumenLoader color="status-error-05" />
    </div>
  ),
};

// IconLoader: generic spinner that spins any icon.

export const DefaultSpinner: Story = {
  render: () => <IconLoader />,
};

export const CustomIcon: Story = {
  render: () => <IconLoader icon={SvgSettings} size={40} />,
};

export const IconColors: Story = {
  render: () => (
    <div className="flex items-end gap-6">
      <IconLoader size={32} />
      <IconLoader size={32} color="text-04" />
      <IconLoader size={32} color="status-success-05" />
    </div>
  ),
};

// color="inherit" applies no class, so the mark takes the ambient text color.
export const Inherit: Story = {
  render: () => (
    <div className="flex items-center gap-6 text-status-error-05">
      <LumenLoader color="inherit" />
      <IconLoader size={32} color="inherit" />
    </div>
  ),
};
