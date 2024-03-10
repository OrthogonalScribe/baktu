# -O3 results in `cap_set_proc: Operation not permitted` even when `getcap`
#  reports `cap_sys_admin=p`
CXXFLAGS=-O2 -Wall -Wextra -Wpedantic
LDLIBS=-lcap

get-all-xattrs: get-all-xattrs.cpp
	$(CXX) -o $@ $(CXXFLAGS) $^ $(LDLIBS)
	sudo setcap cap_sys_admin=p $@

clean:
	rm -f get-all-xattrs
