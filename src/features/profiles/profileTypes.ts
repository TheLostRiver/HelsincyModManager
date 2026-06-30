export type Profile = {
  id: string;
  name: string;
  description: string | null;
  isActive: boolean;
  createdAt: number;
  updatedAt: number;
};

export type CreateProfileInput = {
  name: string;
  description?: string | null;
};

export type UpdateProfileInput = {
  profileId: string;
  name?: string;
  description?: string | null;
};
