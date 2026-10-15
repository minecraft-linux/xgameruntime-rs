extern "C" int strcmp(const char*, const char*);
extern "C" void free(void*);

class type_info {
  type_info& operator=(const type_info&);
  type_info(const type_info&);

  mutable struct {
    const char* undecorated_name;
    const char decorated_name[1];
  } data;

  int compare(const type_info& rhs) const noexcept;

public:
  virtual ~type_info();

  const char* name() const noexcept;
  unsigned long long hash_code() const noexcept;
};

type_info::type_info(const type_info&) = default;

int type_info::compare(const type_info& rhs) const noexcept {
  if (&data == &rhs.data) {
    return 0;
  }
  return strcmp(&data.decorated_name[1], &rhs.data.decorated_name[1]);
}

const char* type_info::name() const noexcept {
  return &data.decorated_name[1];
}

unsigned long long type_info::hash_code() const noexcept {
  return 0;
}

type_info::~type_info() {}

void operator delete(void* p, unsigned long long) noexcept {
  free(p);
}
